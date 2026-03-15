use agent_core::{PlanDetail, TaskExecutionDetail, TaskRecord};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{error::ApiResult, state::ApiState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListTasksQuery {
    session_id: Option<Uuid>,
}

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/tasks", get(list_tasks))
        .route("/tasks/:task_id", get(get_task))
        .route("/tasks/:task_id/plan", get(get_task_plan))
        .route("/tasks/:task_id/execution", get(get_task_execution))
}

async fn list_tasks(
    State(state): State<ApiState>,
    Query(query): Query<ListTasksQuery>,
) -> ApiResult<Vec<TaskRecord>> {
    Ok(Json(state.core.list_tasks(query.session_id).await?))
}

async fn get_task(
    State(state): State<ApiState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<TaskRecord> {
    Ok(Json(state.core.get_task(task_id).await?))
}

async fn get_task_plan(
    State(state): State<ApiState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<PlanDetail> {
    Ok(Json(state.core.get_task_plan(task_id).await?))
}

async fn get_task_execution(
    State(state): State<ApiState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<TaskExecutionDetail> {
    Ok(Json(state.core.get_task_execution(task_id).await?))
}
