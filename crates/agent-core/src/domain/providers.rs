use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ProviderType, ResourceStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountRecord {
    pub id: Uuid,
    pub provider_type: ProviderType,
    pub display_name: String,
    pub base_url: Option<String>,
    pub status: ResourceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub models: Vec<ProviderModelRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelRecord {
    pub id: Uuid,
    pub model_name: String,
    pub context_window: u32,
    pub supports_tools: bool,
    pub supports_embeddings: bool,
    pub capabilities: Vec<String>,
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderRequest {
    pub provider_type: ProviderType,
    pub display_name: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderRequest {
    pub display_name: Option<String>,
    pub base_url: Option<String>,
    pub status: Option<ResourceStatus>,
}
