use agent_core::AgentCore;
use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::state::ApiState;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/", get(root))
        .route("/healthz", get(health))
}

async fn root(State(state): State<ApiState>) -> Json<Value> {
    let core: &AgentCore = &state.core;
    Json(core.root_docs())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
