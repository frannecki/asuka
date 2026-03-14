use std::time::Duration;

use serde_json::json;
use tokio::time::sleep;
use uuid::Uuid;

use crate::{core::AgentCore, error::CoreResult, runtime::fallback_response, storage::RunContext};

impl AgentCore {
    pub(crate) async fn execute_run(&self, session_id: Uuid, run_id: Uuid, user_content: String) {
        self.publish_event(
            "run.started",
            run_id,
            session_id,
            json!({ "status": "running" }),
        );
        sleep(Duration::from_millis(120)).await;
        if !self.run_is_active(run_id).await {
            return;
        }

        let RunContext {
            providers,
            recent_messages,
            memory_hits,
        } = match self
            .store
            .build_run_context(session_id, &user_content)
            .await
        {
            Ok(context) => context,
            Err(error) => {
                self.mark_run_failed(run_id, session_id, error).await;
                return;
            }
        };
        let providers_count = providers.len();
        let provider_selection = self.select_provider_model(&providers);

        if let Err(error) = self
            .apply_provider_selection(run_id, session_id, provider_selection.as_ref())
            .await
        {
            self.mark_run_failed(run_id, session_id, error).await;
            return;
        }
        if !self.run_is_active(run_id).await {
            return;
        }

        self.publish_event(
            "run.step.started",
            run_id,
            session_id,
            json!({
                "stepType": "context-build",
                "message": "Building context from the session, registry, and retrieval layers."
            }),
        );
        sleep(Duration::from_millis(120)).await;
        if !self.run_is_active(run_id).await {
            return;
        }

        self.publish_event(
            "memory.retrieved",
            run_id,
            session_id,
            json!({ "hits": memory_hits }),
        );

        self.maybe_emit_prototype_subagent_activity(run_id, session_id, &user_content)
            .await;
        if !self.run_is_active(run_id).await {
            return;
        }

        let response = match self
            .resolve_response(
                run_id,
                session_id,
                provider_selection.as_ref(),
                &recent_messages,
                &memory_hits,
                &user_content,
                providers_count,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.mark_run_failed(run_id, session_id, error).await;
                return;
            }
        };

        self.stream_response_deltas(run_id, session_id, &response)
            .await;
        if !self.run_is_active(run_id).await {
            return;
        }

        if let Err(error) = self
            .finalize_run(session_id, run_id, &user_content, response)
            .await
        {
            self.mark_run_failed(run_id, session_id, error).await;
        }
    }

    async fn apply_provider_selection(
        &self,
        run_id: Uuid,
        session_id: Uuid,
        selection: Option<&crate::runtime::ProviderSelection>,
    ) -> CoreResult<()> {
        let Some(selection) = selection else {
            return Ok(());
        };

        self.store
            .set_run_selection(
                run_id,
                selection.provider_name.clone(),
                selection.model_name.clone(),
            )
            .await?;

        self.publish_event(
            "model.selected",
            run_id,
            session_id,
            json!({
                "providerId": selection.provider_id,
                "providerName": selection.provider_name,
                "providerType": selection.provider_type,
                "modelName": selection.model_name
            }),
        );

        Ok(())
    }

    async fn resolve_response(
        &self,
        run_id: Uuid,
        session_id: Uuid,
        selection: Option<&crate::runtime::ProviderSelection>,
        recent_messages: &[crate::domain::MessageRecord],
        memory_hits: &[crate::domain::MemorySearchHit],
        user_content: &str,
        providers_count: usize,
    ) -> CoreResult<String> {
        match self
            .resolve_model_or_tool_response(
                selection,
                recent_messages,
                memory_hits,
                user_content,
                session_id,
                run_id,
                providers_count,
            )
            .await
        {
            Ok(response) => Ok(response),
            Err(error) => {
                self.publish_event(
                    "run.step.started",
                    run_id,
                    session_id,
                    json!({
                        "stepType": "model-fallback",
                        "message": error.message
                    }),
                );
                Ok(fallback_response(
                    selection,
                    memory_hits,
                    user_content,
                    providers_count,
                ))
            }
        }
    }

    async fn resolve_model_or_tool_response(
        &self,
        selection: Option<&crate::runtime::ProviderSelection>,
        recent_messages: &[crate::domain::MessageRecord],
        memory_hits: &[crate::domain::MemorySearchHit],
        user_content: &str,
        session_id: Uuid,
        run_id: Uuid,
        providers_count: usize,
    ) -> CoreResult<String> {
        let selection = match selection {
            Some(selection) => selection,
            None => {
                return Ok(fallback_response(
                    None,
                    memory_hits,
                    user_content,
                    providers_count,
                ))
            }
        };

        self.run_tool_loop(
            selection,
            recent_messages,
            memory_hits,
            user_content,
            session_id,
            run_id,
            providers_count,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::test_support::{
        create_test_core_with_openrouter_transport, multi_provider_config_toml, runtime_test_lock,
        EnvVarGuard, TestOpenRouterOutcome, TestOpenRouterTransport,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn resolve_response_falls_back_when_openrouter_transport_errors() {
        let _lock = runtime_test_lock().lock().expect("lock runtime test");
        let _openrouter_key = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Error(
            "simulated upstream failure".to_string(),
        )]);
        let core =
            create_test_core_with_openrouter_transport(multi_provider_config_toml(), transport);

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        let response = core
            .resolve_response(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some(&selection),
                &[],
                &[],
                "Explain fallback behavior",
                providers.len(),
            )
            .await
            .expect("fallback response");

        assert!(response.contains("Selected provider OpenRouter using model"));
        assert!(response.contains("You said: Explain fallback behavior"));
        assert!(response.contains("No long-term memory hits matched strongly enough"));
    }
}
