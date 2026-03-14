use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
pub(super) struct ChromaCollection {
    pub id: String,
}

#[derive(Serialize)]
pub(super) struct ChromaUpsertRequest {
    pub ids: Vec<String>,
    pub embeddings: Vec<Vec<f32>>,
    pub documents: Vec<String>,
    pub metadatas: Vec<Value>,
}

#[derive(Serialize)]
pub(super) struct ChromaQueryRequest {
    pub query_embeddings: Vec<Vec<f32>>,
    pub n_results: usize,
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#where: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct ChromaQueryResponse {
    pub ids: Vec<Vec<String>>,
    #[serde(default)]
    pub documents: Vec<Vec<Option<String>>>,
    #[serde(default)]
    pub metadatas: Vec<Vec<Option<Value>>>,
    #[serde(default)]
    pub distances: Vec<Vec<Option<f32>>>,
}

#[derive(Clone)]
pub(crate) struct ChromaRecord {
    pub id: String,
    pub document: String,
    pub metadata: Value,
    pub embedding: Vec<f32>,
}
