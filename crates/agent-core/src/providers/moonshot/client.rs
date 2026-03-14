use crate::{
    core::AgentCore,
    domain::{MemorySearchHit, MessageRecord},
    error::{CoreError, CoreResult},
    providers::openrouter::client::OpenRouterTransportRequest,
    runtime::ProviderSelection,
};

use super::{
    messages::build_moonshot_messages,
    types::{MoonshotChatRequest, MoonshotChatResponse},
};

impl AgentCore {
    pub(crate) async fn invoke_moonshot(
        &self,
        selection: &ProviderSelection,
        recent_messages: &[MessageRecord],
        memory_hits: &[MemorySearchHit],
        user_content: &str,
    ) -> CoreResult<String> {
        let api_key_env = selection
            .api_key_env
            .clone()
            .ok_or_else(|| CoreError::upstream("Moonshot config is missing api_key_env"))?;
        let api_key = std::env::var(&api_key_env)
            .map_err(|_| CoreError::upstream(format!("{api_key_env} is not available")))?;

        let request_body = MoonshotChatRequest {
            model: selection.model_name.clone(),
            messages: build_moonshot_messages(
                selection,
                recent_messages,
                memory_hits,
                user_content,
            ),
            temperature: 1,
        };
        let response = self
            .openrouter_transport
            .send_chat_completion(OpenRouterTransportRequest {
                endpoint: format!("{}/chat/completions", selection.base_url),
                api_key,
                body: serde_json::to_vec(&request_body).map_err(|error| {
                    CoreError::upstream(format!("invalid request body: {error}"))
                })?,
            })
            .await?;

        if !response.status.is_success() {
            let body = String::from_utf8_lossy(&response.body).into_owned();
            return Err(CoreError::upstream(format!(
                "Moonshot returned {}: {body}",
                response.status
            )));
        }

        let body = serde_json::from_slice::<MoonshotChatResponse>(&response.body)
            .map_err(|error| CoreError::upstream(format!("invalid Moonshot response: {error}")))?;

        body.choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| CoreError::upstream("Moonshot returned an empty completion"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::test_support::{
        create_test_core_with_openrouter_transport, moonshot_provider_config_toml,
        runtime_test_lock, EnvVarGuard, TestOpenRouterOutcome, TestOpenRouterResponse,
        TestOpenRouterTransport,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn invoke_moonshot_returns_completion_and_records_request() {
        let _lock = runtime_test_lock().lock().expect("lock provider test");
        let _moonshot_key = EnvVarGuard::set("MOONSHOT_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Response(
            TestOpenRouterResponse::json(
                200,
                r#"{"choices":[{"message":{"content":"moonshot success"}}]}"#,
            ),
        )]);
        let core = create_test_core_with_openrouter_transport(
            moonshot_provider_config_toml(),
            transport.clone(),
        );

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        let response = core
            .invoke_moonshot(&selection, &[], &[], "Explain provider routing")
            .await
            .expect("provider response");

        assert_eq!(response, "moonshot success");
        let requests = transport.recorded_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].endpoint.ends_with("/chat/completions"));

        let body: Value =
            serde_json::from_slice(&requests[0].body).expect("decode recorded request body");
        assert_eq!(body["model"], selection.model_name);
        assert_eq!(body["temperature"], 1);
        assert_eq!(body["messages"][0]["role"], "system");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invoke_moonshot_maps_non_success_status_to_upstream_error() {
        let _lock = runtime_test_lock().lock().expect("lock provider test");
        let _moonshot_key = EnvVarGuard::set("MOONSHOT_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Response(
            TestOpenRouterResponse::json(400, r#"{"error":"bad request"}"#),
        )]);
        let core =
            create_test_core_with_openrouter_transport(moonshot_provider_config_toml(), transport);

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        let error = core
            .invoke_moonshot(&selection, &[], &[], "Explain provider routing")
            .await
            .expect_err("upstream error");

        assert_eq!(error.status, 502);
        assert!(error.message.contains("Moonshot returned 400 Bad Request"));
        assert!(error.message.contains("bad request"));
    }
}
