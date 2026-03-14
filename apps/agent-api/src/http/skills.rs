use agent_core::{CreateSkillRequest, SkillRecord, UpdateSkillRequest};
use axum::{
    extract::{Path, State},
    routing::{get, patch},
    Json, Router,
};
use uuid::Uuid;

use crate::{error::ApiResult, state::ApiState};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/skills", get(list_skills).post(create_skill))
        .route("/skills/:skill_id", patch(update_skill))
}

async fn list_skills(State(state): State<ApiState>) -> ApiResult<Vec<SkillRecord>> {
    Ok(Json(state.core.list_skills().await?))
}

async fn create_skill(
    State(state): State<ApiState>,
    Json(payload): Json<CreateSkillRequest>,
) -> ApiResult<SkillRecord> {
    Ok(Json(state.core.create_skill(payload).await?))
}

async fn update_skill(
    State(state): State<ApiState>,
    Path(skill_id): Path<Uuid>,
    Json(payload): Json<UpdateSkillRequest>,
) -> ApiResult<SkillRecord> {
    Ok(Json(state.core.update_skill(skill_id, payload).await?))
}
