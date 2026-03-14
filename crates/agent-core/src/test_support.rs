use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use async_trait::async_trait;
use http::StatusCode;
use uuid::Uuid;

use crate::{
    config::ModelsConfig,
    core::AgentCore,
    error::{CoreError, CoreResult},
    providers::openrouter::client::{
        OpenRouterTransport, OpenRouterTransportRequest, OpenRouterTransportResponse,
    },
    storage::InMemoryStore,
};

pub fn runtime_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    pub fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }

    pub fn remove(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::remove_var(key);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(previous) => std::env::set_var(self.key, previous),
            None => std::env::remove_var(self.key),
        }
    }
}

pub fn write_test_models_config(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("asuka-models-{}.toml", Uuid::new_v4()));
    fs::write(&path, contents).expect("write test models config");
    path
}

pub fn create_test_core(config_toml: &str) -> AgentCore {
    let config_path = write_test_models_config(config_toml);
    let config: ModelsConfig = toml::from_str(config_toml).expect("parse test models config");
    let store = Arc::new(InMemoryStore::seeded(&config));
    AgentCore::with_store(config_path, store).expect("build test core")
}

pub fn create_test_core_with_openrouter_transport(
    config_toml: &str,
    openrouter_transport: Arc<TestOpenRouterTransport>,
) -> AgentCore {
    let config_path = write_test_models_config(config_toml);
    let config: ModelsConfig = toml::from_str(config_toml).expect("parse test models config");
    let store = Arc::new(InMemoryStore::seeded(&config));
    AgentCore::with_store_and_openrouter_transport(config_path, store, openrouter_transport)
        .expect("build test core with transport")
}

#[derive(Debug, Clone)]
pub struct TestOpenRouterRequest {
    pub endpoint: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TestOpenRouterResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl TestOpenRouterResponse {
    pub fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            body: body.as_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TestOpenRouterOutcome {
    Response(TestOpenRouterResponse),
    Error(String),
}

pub struct TestOpenRouterTransport {
    requests: Mutex<Vec<TestOpenRouterRequest>>,
    responses: Mutex<VecDeque<TestOpenRouterOutcome>>,
}

impl TestOpenRouterTransport {
    pub fn new(responses: Vec<TestOpenRouterOutcome>) -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::from(responses)),
        })
    }

    pub fn recorded_requests(&self) -> Vec<TestOpenRouterRequest> {
        self.requests
            .lock()
            .expect("lock test transport requests")
            .clone()
    }
}

#[async_trait]
impl OpenRouterTransport for TestOpenRouterTransport {
    async fn send_chat_completion(
        &self,
        request: OpenRouterTransportRequest,
    ) -> CoreResult<OpenRouterTransportResponse> {
        self.requests
            .lock()
            .expect("lock test transport requests")
            .push(TestOpenRouterRequest {
                endpoint: request.endpoint,
                body: request.body,
            });

        self.responses
            .lock()
            .expect("lock test transport responses")
            .pop_front()
            .map(|outcome| match outcome {
                TestOpenRouterOutcome::Response(response) => Ok(OpenRouterTransportResponse {
                    status: StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK),
                    body: response.body,
                }),
                TestOpenRouterOutcome::Error(message) => Err(CoreError::upstream(message)),
            })
            .unwrap_or_else(|| {
                Err(CoreError::upstream(format!(
                    "no queued OpenRouter response (default status {})",
                    StatusCode::INTERNAL_SERVER_ERROR
                )))
            })
    }
}

pub fn multi_provider_config_toml() -> &'static str {
    r#"
[[providers]]
provider_type = "openRouter"
display_name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
default_model = "nvidia/nemotron-3-super-120b-a12b:free"

[[providers.models]]
name = "nvidia/nemotron-3-super-120b-a12b:free"
context_window = 131072
supports_tools = false
supports_embeddings = false
capabilities = ["chat"]

[[providers]]
provider_type = "openAi"
display_name = "OpenAI"
base_url = "https://api.openai.com/v1"
default_model = "gpt-4.1"

[[providers.models]]
name = "gpt-4.1"
context_window = 128000
supports_tools = true
supports_embeddings = false
capabilities = ["chat", "tools"]
"#
}

pub fn moonshot_provider_config_toml() -> &'static str {
    r#"
[[providers]]
provider_type = "moonshot"
display_name = "Moonshot"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
default_model = "kimi-k2.5"

[[providers.models]]
name = "kimi-k2.5"
context_window = 131072
supports_tools = false
supports_embeddings = false
capabilities = ["chat", "reasoning"]
"#
}
