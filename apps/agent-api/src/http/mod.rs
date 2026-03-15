pub mod artifacts;
pub mod mcp;
pub mod memory;
pub mod providers;
pub mod root;
pub mod runs;
pub mod sessions;
pub mod skills;
pub mod sse;
pub mod subagents;
pub mod tasks;
pub mod workspace;

use axum::Router;

use crate::state::ApiState;

pub fn api_router() -> Router<ApiState> {
    Router::new()
        .merge(sessions::router())
        .merge(tasks::router())
        .merge(runs::router())
        .merge(artifacts::router())
        .merge(skills::router())
        .merge(subagents::router())
        .merge(providers::router())
        .merge(memory::router())
        .merge(mcp::router())
        .merge(workspace::router())
}
