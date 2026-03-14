use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct MoonshotChatRequest {
    pub model: String,
    pub messages: Vec<MoonshotMessage>,
    pub temperature: u8,
}

#[derive(Serialize)]
pub(crate) struct MoonshotMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub(crate) struct MoonshotChatResponse {
    pub choices: Vec<MoonshotChoice>,
}

#[derive(Deserialize)]
pub(crate) struct MoonshotChoice {
    pub message: MoonshotChoiceMessage,
}

#[derive(Deserialize)]
pub(crate) struct MoonshotChoiceMessage {
    pub content: String,
}
