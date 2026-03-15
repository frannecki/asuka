use agent_core::WorkspaceNode;
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse},
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
        .route(
            "/sessions/:session_id/workspace/tree",
            get(get_workspace_tree),
        )
        .route(
            "/sessions/:session_id/workspace/raw/*path",
            get(get_workspace_raw),
        )
        .route(
            "/sessions/:session_id/workspace/render/*path",
            get(render_workspace_markdown),
        )
}

async fn get_workspace_tree(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<WorkspaceNode> {
    Ok(Json(
        state.core.get_session_workspace_tree(session_id).await?,
    ))
}

async fn get_workspace_raw(
    State(state): State<ApiState>,
    Path((session_id, path)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let body = state
        .core
        .read_session_workspace_file(session_id, &path)
        .await?;
    Ok((
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(content_type_for_path(&path)),
        )],
        body,
    ))
}

async fn render_workspace_markdown(
    State(state): State<ApiState>,
    Path((session_id, path)): Path<(Uuid, String)>,
) -> Result<Html<String>, ApiError> {
    Ok(Html(
        state
            .core
            .render_session_workspace_markdown(session_id, &path)
            .await?,
    ))
}

fn content_type_for_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "text/plain; charset=utf-8",
    }
}
