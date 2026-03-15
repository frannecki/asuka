use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{MessageRecord, RunStatus};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunStreamStatus {
    Idle,
    Active,
    Completed,
    Failed,
    Cancelled,
}

impl Default for RunStreamStatus {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub task_id: Uuid,
    pub trigger_type: String,
    pub status: RunStatus,
    pub selected_provider: Option<String>,
    pub selected_model: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    #[serde(default)]
    pub effective_skill_names: Vec<String>,
    #[serde(default)]
    pub pinned_skill_names: Vec<String>,
    #[serde(default)]
    pub last_event_sequence: u64,
    #[serde(default)]
    pub stream_status: RunStreamStatus,
    #[serde(default)]
    pub active_stream_message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAccepted {
    pub run: RunRecord,
    pub user_message: MessageRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventEnvelope {
    pub event_type: String,
    pub run_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRunEnvelope {
    pub run: Option<RunRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventHistory {
    pub run_id: Uuid,
    pub after_sequence: Option<u64>,
    pub events: Vec<RunEventEnvelope>,
    pub last_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckpointSummary {
    pub run_id: Uuid,
    pub last_sequence: u64,
    pub draft_reply_text: String,
    pub updated_at: DateTime<Utc>,
    pub active_stream_message_id: Option<Uuid>,
}
