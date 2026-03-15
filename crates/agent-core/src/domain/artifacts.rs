use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    Report,
    Response,
    Data,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactRenderMode {
    Html,
    Markdown,
    Json,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactProducerKind {
    Run,
    RunStep,
    ToolInvocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub path: String,
    pub display_name: String,
    pub description: String,
    pub kind: ArtifactKind,
    pub media_type: String,
    pub render_mode: ArtifactRenderMode,
    pub size_bytes: u64,
    pub producer_kind: Option<ArtifactProducerKind>,
    pub producer_ref_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
