use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ResourceStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentRecord {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub scope: String,
    pub max_steps: u32,
    pub status: ResourceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubagentRequest {
    pub name: String,
    pub description: String,
    pub scope: String,
    pub max_steps: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubagentRequest {
    pub description: Option<String>,
    pub scope: Option<String>,
    pub max_steps: Option<u32>,
    pub status: Option<ResourceStatus>,
}
