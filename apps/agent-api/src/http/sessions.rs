use agent_core::{
    ActiveRunEnvelope, CreateSessionRequest, MessageRecord, PostMessageRequest, RunAccepted,
    SessionDetail, SessionRecord, UpdateSessionRequest,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    state::ApiState,
};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/sessions", get(list_sessions).post(create_session))
        .route(
            "/sessions/:session_id",
            get(get_session)
                .patch(update_session)
                .delete(delete_session),
        )
        .route(
            "/sessions/:session_id/messages",
            get(list_messages).post(post_message),
        )
        .route("/sessions/:session_id/active-run", get(get_active_run))
}

async fn list_sessions(State(state): State<ApiState>) -> ApiResult<Vec<SessionRecord>> {
    Ok(Json(state.core.list_sessions().await?))
}

async fn create_session(
    State(state): State<ApiState>,
    Json(payload): Json<CreateSessionRequest>,
) -> ApiResult<SessionRecord> {
    Ok(Json(state.core.create_session(payload).await?))
}

async fn get_session(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<SessionDetail> {
    Ok(Json(state.core.get_session(session_id).await?))
}

async fn update_session(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<UpdateSessionRequest>,
) -> ApiResult<SessionRecord> {
    Ok(Json(state.core.update_session(session_id, payload).await?))
}

async fn delete_session(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.core.delete_session(session_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_messages(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Vec<MessageRecord>> {
    Ok(Json(state.core.list_messages(session_id).await?))
}

async fn post_message(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<PostMessageRequest>,
) -> ApiResult<RunAccepted> {
    Ok(Json(state.core.post_message(session_id, payload).await?))
}

async fn get_active_run(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<ActiveRunEnvelope> {
    Ok(Json(state.core.get_active_run(session_id).await?))
}
