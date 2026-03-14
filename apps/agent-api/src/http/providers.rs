use agent_core::{
    CreateProviderRequest, ProviderAccountRecord, ProviderModelRecord, TestResult,
    UpdateProviderRequest,
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
        .route("/providers", get(list_providers).post(create_provider))
        .route(
            "/providers/:provider_id",
            get(get_provider).patch(update_provider),
        )
        .route("/providers/:provider_id/test", post(test_provider))
        .route("/providers/:provider_id/models", get(list_provider_models))
        .route(
            "/providers/:provider_id/models/sync",
            post(sync_provider_models),
        )
}

async fn list_providers(State(state): State<ApiState>) -> ApiResult<Vec<ProviderAccountRecord>> {
    Ok(Json(state.core.list_providers().await?))
}

async fn create_provider(
    State(state): State<ApiState>,
    Json(payload): Json<CreateProviderRequest>,
) -> ApiResult<ProviderAccountRecord> {
    Ok(Json(state.core.create_provider(payload).await?))
}

async fn get_provider(
    State(state): State<ApiState>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<ProviderAccountRecord> {
    Ok(Json(state.core.get_provider(provider_id).await?))
}

async fn update_provider(
    State(state): State<ApiState>,
    Path(provider_id): Path<Uuid>,
    Json(payload): Json<UpdateProviderRequest>,
) -> ApiResult<ProviderAccountRecord> {
    Ok(Json(
        state.core.update_provider(provider_id, payload).await?,
    ))
}

async fn test_provider(
    State(state): State<ApiState>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<TestResult> {
    Ok(Json(state.core.test_provider(provider_id).await?))
}

async fn list_provider_models(
    State(state): State<ApiState>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Vec<ProviderModelRecord>> {
    Ok(Json(state.core.list_provider_models(provider_id).await?))
}

async fn sync_provider_models(
    State(state): State<ApiState>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<ProviderAccountRecord> {
    Ok(Json(state.core.sync_provider_models(provider_id).await?))
}
