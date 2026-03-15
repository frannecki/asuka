use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MemoryScope {
    Session,
    Project,
    Global,
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self::Global
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDocumentRecord {
    pub id: Uuid,
    pub title: String,
    pub namespace: String,
    pub source: String,
    #[serde(default)]
    pub memory_scope: MemoryScope,
    #[serde(default)]
    pub owner_session_id: Option<Uuid>,
    #[serde(default)]
    pub owner_task_id: Option<Uuid>,
    #[serde(default)]
    pub is_pinned: bool,
    pub content: String,
    pub summary: String,
    pub chunk_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryChunkRecord {
    pub id: Uuid,
    pub document_id: Uuid,
    pub namespace: String,
    pub ordinal: usize,
    pub content: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchHit {
    pub document_id: Uuid,
    pub chunk_id: Uuid,
    pub document_title: String,
    pub namespace: String,
    pub memory_scope: MemoryScope,
    pub owner_session_id: Option<Uuid>,
    pub content: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchResult {
    pub hits: Vec<MemorySearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReindexResult {
    pub documents: usize,
    pub chunks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDocumentDetail {
    pub document: MemoryDocumentRecord,
    pub chunks: Vec<MemoryChunkRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMemoryDocumentRequest {
    pub title: String,
    pub namespace: Option<String>,
    pub source: Option<String>,
    pub memory_scope: Option<MemoryScope>,
    pub owner_session_id: Option<Uuid>,
    pub owner_task_id: Option<Uuid>,
    pub is_pinned: Option<bool>,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchRequest {
    pub query: String,
    pub namespace: Option<String>,
    pub memory_scopes: Option<Vec<MemoryScope>>,
    pub owner_session_id: Option<Uuid>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemoryDocumentRequest {
    pub title: Option<String>,
    pub namespace: Option<String>,
    pub memory_scope: Option<MemoryScope>,
    pub owner_session_id: Option<Uuid>,
    pub is_pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMemoryRetrievalRecord {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub hits: Vec<MemorySearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMemoryOverview {
    pub session_id: Uuid,
    pub short_term_summary: String,
    pub scoped_documents: Vec<MemoryDocumentRecord>,
    pub pinned_documents: Vec<MemoryDocumentRecord>,
    pub recent_retrievals: Vec<SessionMemoryRetrievalRecord>,
}
