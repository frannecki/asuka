use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct OpenRouterChatRequest {
    pub model: String,
    pub messages: Vec<OpenRouterMessage>,
    pub temperature: f32,
}

#[derive(Serialize)]
pub(crate) struct OpenRouterMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatResponse {
    pub choices: Vec<OpenRouterChoice>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChoice {
    pub message: OpenRouterChoiceMessage,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChoiceMessage {
    pub content: String,
}
