use agent_core::{
    ApplySkillPresetRequest, CreateSkillRequest, ReplaceSessionSkillsRequest, SessionSkillsDetail,
    SkillPreset, SkillRecord, UpdateSessionSkillBindingRequest, UpdateSkillRequest,
};
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
        .route("/skill-presets", get(list_skill_presets))
        .route(
            "/sessions/:session_id/skills",
            get(get_session_skills).put(replace_session_skills),
        )
        .route(
            "/sessions/:session_id/skills/:skill_id",
            patch(update_session_skill_binding),
        )
        .route(
            "/sessions/:session_id/skills/apply-preset",
            post_apply_session_skill_preset(),
        )
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

async fn list_skill_presets(State(state): State<ApiState>) -> ApiResult<Vec<SkillPreset>> {
    Ok(Json(state.core.list_skill_presets().await?))
}

async fn get_session_skills(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionSkillsDetail> {
    Ok(Json(state.core.get_session_skills(session_id).await?))
}

async fn replace_session_skills(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<ReplaceSessionSkillsRequest>,
) -> ApiResult<SessionSkillsDetail> {
    Ok(Json(
        state
            .core
            .replace_session_skills(session_id, payload)
            .await?,
    ))
}

async fn update_session_skill_binding(
    State(state): State<ApiState>,
    Path((session_id, skill_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateSessionSkillBindingRequest>,
) -> ApiResult<SessionSkillsDetail> {
    Ok(Json(
        state
            .core
            .update_session_skill_binding(session_id, skill_id, payload)
            .await?,
    ))
}

async fn apply_session_skill_preset(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<ApplySkillPresetRequest>,
) -> ApiResult<SessionSkillsDetail> {
    Ok(Json(
        state
            .core
            .apply_session_skill_preset(session_id, payload)
            .await?,
    ))
}

fn post_apply_session_skill_preset() -> axum::routing::MethodRouter<ApiState> {
    axum::routing::post(apply_session_skill_preset)
}
