use async_trait::async_trait;
use http::StatusCode;
use hyper::{body::to_bytes, Body, Client, Request};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};

use crate::{
    core::AgentCore,
    domain::{MemorySearchHit, MessageRecord, ProviderType},
    error::{CoreError, CoreResult},
    runtime::ProviderSelection,
};

use super::{
    messages::build_openrouter_messages,
    types::{OpenRouterChatRequest, OpenRouterChatResponse},
};

#[derive(Debug, Clone)]
pub(crate) struct OpenRouterTransportRequest {
    pub endpoint: String,
    pub api_key: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenRouterTransportResponse {
    pub status: StatusCode,
    pub body: Vec<u8>,
}

#[async_trait]
pub(crate) trait OpenRouterTransport: Send + Sync {
    async fn send_chat_completion(
        &self,
        request: OpenRouterTransportRequest,
    ) -> CoreResult<OpenRouterTransportResponse>;
}

pub(crate) struct HyperOpenRouterTransport {
    http_client: Client<HttpsConnector<hyper::client::HttpConnector>, Body>,
}

impl HyperOpenRouterTransport {
    pub(crate) fn new() -> Self {
        let connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build();

        Self {
            http_client: Client::builder().build(connector),
        }
    }
}

#[async_trait]
impl OpenRouterTransport for HyperOpenRouterTransport {
    async fn send_chat_completion(
        &self,
        request: OpenRouterTransportRequest,
    ) -> CoreResult<OpenRouterTransportResponse> {
        let request = Request::builder()
            .method("POST")
            .uri(&request.endpoint)
            .header("Authorization", format!("Bearer {}", request.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://asuka.local")
            .header("X-Title", "Asuka Agent")
            .body(Body::from(request.body))
            .map_err(|error| CoreError::upstream(format!("failed to build request: {error}")))?;

        let response =
            self.http_client.request(request).await.map_err(|error| {
                CoreError::upstream(format!("OpenRouter request failed: {error}"))
            })?;
        let status = response.status();
        let body = to_bytes(response.into_body()).await.map_err(|error| {
            CoreError::upstream(format!("failed to read OpenRouter response: {error}"))
        })?;

        Ok(OpenRouterTransportResponse {
            status,
            body: body.to_vec(),
        })
    }
}

impl AgentCore {
    pub(crate) async fn invoke_openrouter(
        &self,
        selection: &ProviderSelection,
        recent_messages: &[MessageRecord],
        memory_hits: &[MemorySearchHit],
        effective_skill_names: &[String],
        pinned_skill_names: &[String],
        user_content: &str,
    ) -> CoreResult<String> {
        let api_key_env = selection
            .api_key_env
            .clone()
            .ok_or_else(|| CoreError::upstream("OpenRouter config is missing api_key_env"))?;
        let api_key = std::env::var(&api_key_env)
            .map_err(|_| CoreError::upstream(format!("{api_key_env} is not available")))?;

        let request_body = OpenRouterChatRequest {
            model: selection.model_name.clone(),
            messages: build_openrouter_messages(
                selection,
                recent_messages,
                memory_hits,
                effective_skill_names,
                pinned_skill_names,
                user_content,
            ),
            temperature: 0.2,
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
                "OpenRouter returned {}: {body}",
                response.status
            )));
        }

        let body =
            serde_json::from_slice::<OpenRouterChatResponse>(&response.body).map_err(|error| {
                CoreError::upstream(format!("invalid OpenRouter response: {error}"))
            })?;

        body.choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| CoreError::upstream("OpenRouter returned an empty completion"))
    }

    pub(crate) async fn generate_response(
        &self,
        selection: Option<&ProviderSelection>,
        recent_messages: &[MessageRecord],
        memory_hits: &[MemorySearchHit],
        effective_skill_names: &[String],
        pinned_skill_names: &[String],
        user_content: &str,
        providers_count: usize,
    ) -> CoreResult<String> {
        let selection = match selection {
            Some(selection) => selection,
            None => {
                return Ok(crate::runtime::fallback_response(
                    None,
                    memory_hits,
                    user_content,
                    providers_count,
                ))
            }
        };

        match selection.provider_type {
            ProviderType::Moonshot => {
                self.invoke_moonshot(
                    selection,
                    recent_messages,
                    memory_hits,
                    effective_skill_names,
                    pinned_skill_names,
                    user_content,
                )
                .await
            }
            ProviderType::OpenRouter => {
                self.invoke_openrouter(
                    selection,
                    recent_messages,
                    memory_hits,
                    effective_skill_names,
                    pinned_skill_names,
                    user_content,
                )
                .await
            }
            _ => Ok(crate::runtime::fallback_response(
                Some(selection),
                memory_hits,
                user_content,
                providers_count,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::test_support::{
        create_test_core_with_openrouter_transport, multi_provider_config_toml, runtime_test_lock,
        EnvVarGuard, TestOpenRouterOutcome, TestOpenRouterResponse, TestOpenRouterTransport,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn invoke_openrouter_returns_completion_and_records_request() {
        let _lock = runtime_test_lock().lock().expect("lock provider test");
        let _openrouter_key = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Response(
            TestOpenRouterResponse::json(
                200,
                r#"{"choices":[{"message":{"content":"provider success"}}]}"#,
            ),
        )]);
        let core = create_test_core_with_openrouter_transport(
            multi_provider_config_toml(),
            transport.clone(),
        );

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        let response = core
            .invoke_openrouter(&selection, &[], &[], &[], &[], "Explain provider routing")
            .await
            .expect("provider response");

        assert_eq!(response, "provider success");
        let requests = transport.recorded_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].endpoint.ends_with("/chat/completions"));

        let body: Value =
            serde_json::from_slice(&requests[0].body).expect("decode recorded request body");
        assert_eq!(body["model"], selection.model_name);
        assert_eq!(body["messages"][0]["role"], "system");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invoke_openrouter_maps_non_success_status_to_upstream_error() {
        let _lock = runtime_test_lock().lock().expect("lock provider test");
        let _openrouter_key = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Response(
            TestOpenRouterResponse::json(429, r#"{"error":"rate limited"}"#),
        )]);
        let core =
            create_test_core_with_openrouter_transport(multi_provider_config_toml(), transport);

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        let error = core
            .invoke_openrouter(&selection, &[], &[], &[], &[], "Explain provider routing")
            .await
            .expect_err("upstream error");

        assert_eq!(error.status, 502);
        assert!(error
            .message
            .contains("OpenRouter returned 429 Too Many Requests"));
        assert!(error.message.contains("rate limited"));
    }
}
