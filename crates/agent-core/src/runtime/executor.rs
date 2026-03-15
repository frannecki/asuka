use std::time::Duration;

use serde_json::json;
use tokio::time::sleep;
use uuid::Uuid;

use crate::{
    core::AgentCore, domain::PlanStepKind, error::CoreResult, memory::summarize_text,
    runtime::fallback_response, storage::RunContext,
};

impl AgentCore {
    pub(crate) async fn execute_run(&self, session_id: Uuid, run_id: Uuid, user_content: String) {
        self.publish_event(
            "run.started",
            run_id,
            session_id,
            json!({ "status": "running" }),
        )
        .await;
        sleep(Duration::from_millis(120)).await;
        if !self.run_is_active(run_id).await {
            return;
        }
        let run = match self.get_run(run_id).await {
            Ok(run) => run,
            Err(_) => return,
        };
        let (context_plan_step_id, respond_plan_step_id) =
            self.lookup_default_plan_step_ids(run.task_id).await;

        let context_step = match self
            .store
            .start_run_step(
                run_id,
                context_plan_step_id,
                PlanStepKind::ContextBuild,
                "Build context".to_string(),
                summarize_text(&user_content, 24),
            )
            .await
        {
            Ok(step) => step,
            Err(error) => {
                self.mark_run_failed(run_id, session_id, error).await;
                return;
            }
        };

        let RunContext {
            providers,
            recent_messages,
            memory_hits,
            effective_skill_names,
            pinned_skill_names,
        } = match self
            .store
            .build_run_context(session_id, &user_content)
            .await
        {
            Ok(context) => {
                if let Err(error) = self
                    .store
                    .complete_run_step(
                        context_step.id,
                        format!(
                            "Loaded {} recent messages and {} memory hits.",
                            context.recent_messages.len(),
                            context.memory_hits.len()
                        ),
                    )
                    .await
                {
                    self.mark_run_failed(run_id, session_id, error).await;
                    return;
                }
                context
            }
            Err(error) => {
                let _ = self
                    .store
                    .fail_run_step(context_step.id, error.message.clone())
                    .await;
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
        )
        .await;
        sleep(Duration::from_millis(120)).await;
        if !self.run_is_active(run_id).await {
            return;
        }

        self.publish_event(
            "memory.retrieved",
            run_id,
            session_id,
            json!({ "hits": memory_hits }),
        )
        .await;

        if let Err(error) = self
            .maybe_emit_prototype_subagent_activity(run_id, session_id, run.task_id, &user_content)
            .await
        {
            self.mark_run_failed(run_id, session_id, error).await;
            return;
        }
        if !self.run_is_active(run_id).await {
            return;
        }

        let response_step = match self
            .store
            .start_run_step(
                run_id,
                respond_plan_step_id,
                PlanStepKind::Respond,
                "Produce response".to_string(),
                summarize_text(&user_content, 24),
            )
            .await
        {
            Ok(step) => step,
            Err(error) => {
                self.mark_run_failed(run_id, session_id, error).await;
                return;
            }
        };

        let response = match self
            .resolve_response(
                run_id,
                session_id,
                provider_selection.as_ref(),
                &recent_messages,
                &memory_hits,
                &effective_skill_names,
                &pinned_skill_names,
                &user_content,
                providers_count,
            )
            .await
        {
            Ok(response) => {
                if let Err(error) = self
                    .store
                    .complete_run_step(response_step.id, summarize_text(&response, 24))
                    .await
                {
                    self.mark_run_failed(run_id, session_id, error).await;
                    return;
                }
                response
            }
            Err(error) => {
                let _ = self
                    .store
                    .fail_run_step(response_step.id, error.message.clone())
                    .await;
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

    async fn lookup_default_plan_step_ids(&self, task_id: Uuid) -> (Option<Uuid>, Option<Uuid>) {
        let Ok(plan_detail) = self.get_task_plan(task_id).await else {
            return (None, None);
        };
        let context = find_plan_step_id(&plan_detail.steps, PlanStepKind::ContextBuild);
        let respond = find_plan_step_id(&plan_detail.steps, PlanStepKind::Respond);
        (context, respond)
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
        )
        .await;

        Ok(())
    }

    async fn resolve_response(
        &self,
        run_id: Uuid,
        session_id: Uuid,
        selection: Option<&crate::runtime::ProviderSelection>,
        recent_messages: &[crate::domain::MessageRecord],
        memory_hits: &[crate::domain::MemorySearchHit],
        effective_skill_names: &[String],
        pinned_skill_names: &[String],
        user_content: &str,
        providers_count: usize,
    ) -> CoreResult<String> {
        match self
            .resolve_model_or_tool_response(
                selection,
                recent_messages,
                memory_hits,
                effective_skill_names,
                pinned_skill_names,
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
                )
                .await;
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
        effective_skill_names: &[String],
        pinned_skill_names: &[String],
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
            effective_skill_names,
            pinned_skill_names,
            user_content,
            session_id,
            run_id,
            providers_count,
        )
        .await
    }
}

fn find_plan_step_id(steps: &[crate::domain::PlanStepRecord], kind: PlanStepKind) -> Option<Uuid> {
    steps
        .iter()
        .find(|step| step.kind == kind)
        .map(|step| step.id)
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
