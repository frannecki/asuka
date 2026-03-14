use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDocumentRecord {
    pub id: Uuid,
    pub title: String,
    pub namespace: String,
    pub source: String,
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
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchRequest {
    pub query: String,
    pub namespace: Option<String>,
    pub limit: Option<usize>,
}
