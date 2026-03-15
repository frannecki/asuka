use agent_core::ArtifactRecord;
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::{error::ApiResult, state::ApiState};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route(
            "/sessions/:session_id/artifacts",
            get(list_session_artifacts),
        )
        .route("/tasks/:task_id/artifacts", get(list_task_artifacts))
        .route("/runs/:run_id/artifacts", get(list_run_artifacts))
}

async fn list_session_artifacts(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Vec<ArtifactRecord>> {
    Ok(Json(state.core.list_session_artifacts(session_id).await?))
}

async fn list_task_artifacts(
    State(state): State<ApiState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Vec<ArtifactRecord>> {
    Ok(Json(state.core.list_task_artifacts(task_id).await?))
}

async fn list_run_artifacts(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Vec<ArtifactRecord>> {
    Ok(Json(state.core.list_run_artifacts(run_id).await?))
}
