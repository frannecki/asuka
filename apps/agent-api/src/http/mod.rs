pub mod mcp;
pub mod memory;
pub mod providers;
pub mod root;
pub mod runs;
pub mod sessions;
pub mod skills;
pub mod sse;
pub mod subagents;

use axum::Router;

use crate::state::ApiState;

pub fn api_router() -> Router<ApiState> {
    Router::new()
        .merge(sessions::router())
        .merge(runs::router())
        .merge(skills::router())
        .merge(subagents::router())
        .merge(providers::router())
        .merge(memory::router())
        .merge(mcp::router())
}
