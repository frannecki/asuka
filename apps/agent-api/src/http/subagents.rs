use agent_core::{CreateSubagentRequest, SubagentRecord, UpdateSubagentRequest};
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::{error::ApiResult, state::ApiState};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/subagents", get(list_subagents).post(create_subagent))
        .route(
            "/subagents/:subagent_id",
            get(get_subagent).patch(update_subagent),
        )
}

async fn list_subagents(State(state): State<ApiState>) -> ApiResult<Vec<SubagentRecord>> {
    Ok(Json(state.core.list_subagents().await?))
}

async fn create_subagent(
    State(state): State<ApiState>,
    Json(payload): Json<CreateSubagentRequest>,
) -> ApiResult<SubagentRecord> {
    Ok(Json(state.core.create_subagent(payload).await?))
}

async fn get_subagent(
    State(state): State<ApiState>,
    Path(subagent_id): Path<Uuid>,
) -> ApiResult<SubagentRecord> {
    Ok(Json(state.core.get_subagent(subagent_id).await?))
}

async fn update_subagent(
    State(state): State<ApiState>,
    Path(subagent_id): Path<Uuid>,
    Json(payload): Json<UpdateSubagentRequest>,
) -> ApiResult<SubagentRecord> {
    Ok(Json(
        state.core.update_subagent(subagent_id, payload).await?,
    ))
}
