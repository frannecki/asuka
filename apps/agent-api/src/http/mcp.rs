use agent_core::{CapabilityEnvelope, CreateMcpServerRequest, McpServerRecord, TestResult};
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
            "/mcp/servers",
            get(list_mcp_servers).post(create_mcp_server),
        )
        .route("/mcp/servers/:server_id", get(get_mcp_server))
        .route("/mcp/servers/:server_id/test", post(test_mcp_server))
        .route(
            "/mcp/servers/:server_id/capabilities",
            get(get_mcp_capabilities),
        )
}

async fn list_mcp_servers(State(state): State<ApiState>) -> ApiResult<Vec<McpServerRecord>> {
    Ok(Json(state.core.list_mcp_servers().await?))
}

async fn create_mcp_server(
    State(state): State<ApiState>,
    Json(payload): Json<CreateMcpServerRequest>,
) -> ApiResult<McpServerRecord> {
    Ok(Json(state.core.create_mcp_server(payload).await?))
}

async fn get_mcp_server(
    State(state): State<ApiState>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<McpServerRecord> {
    Ok(Json(state.core.get_mcp_server(server_id).await?))
}

async fn test_mcp_server(
    State(state): State<ApiState>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<TestResult> {
    Ok(Json(state.core.test_mcp_server(server_id).await?))
}

async fn get_mcp_capabilities(
    State(state): State<ApiState>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<CapabilityEnvelope> {
    Ok(Json(state.core.get_mcp_capabilities(server_id).await?))
}
