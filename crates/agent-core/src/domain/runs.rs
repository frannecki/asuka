use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{MessageRecord, RunStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub trigger_type: String,
    pub status: RunStatus,
    pub selected_provider: Option<String>,
    pub selected_model: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAccepted {
    pub run: RunRecord,
    pub user_message: MessageRecord,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventEnvelope {
    pub event_type: String,
    pub run_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
    pub payload: Value,
}
