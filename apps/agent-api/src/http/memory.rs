use agent_core::{
    CreateMemoryDocumentRequest, MemoryDocumentDetail, MemoryDocumentRecord, MemorySearchRequest,
    MemorySearchResult, ReindexResult,
};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::{error::ApiResult, state::ApiState};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route(
            "/memory/documents",
            get(list_memory_documents).post(create_memory_document),
        )
        .route("/memory/documents/:document_id", get(get_memory_document))
        .route("/memory/search", post(search_memory))
        .route("/memory/reindex", post(reindex_memory))
}

async fn list_memory_documents(
    State(state): State<ApiState>,
) -> ApiResult<Vec<MemoryDocumentRecord>> {
    Ok(Json(state.core.list_memory_documents().await?))
}

async fn create_memory_document(
    State(state): State<ApiState>,
    Json(payload): Json<CreateMemoryDocumentRequest>,
) -> ApiResult<MemoryDocumentRecord> {
    Ok(Json(state.core.create_memory_document(payload).await?))
}

async fn get_memory_document(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
) -> ApiResult<MemoryDocumentDetail> {
    Ok(Json(state.core.get_memory_document(document_id).await?))
}

async fn search_memory(
    State(state): State<ApiState>,
    Json(payload): Json<MemorySearchRequest>,
) -> ApiResult<MemorySearchResult> {
    Ok(Json(state.core.search_memory(payload).await?))
}

async fn reindex_memory(State(state): State<ApiState>) -> ApiResult<ReindexResult> {
    Ok(Json(state.core.reindex_memory().await?))
}
