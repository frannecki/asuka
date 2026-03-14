use std::net::SocketAddr;

use agent_core::AgentCore;
use tracing::info;

mod app;
mod error;
mod http;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "agent_api=info,tower_http=info".into()),
        )
        .init();

    let config_path = std::env::var("MODELS_CONFIG_PATH")
        .unwrap_or_else(|_| "/home/frank/Documents/LLM/agents/asuka/config/models.toml".into());
    let core = AgentCore::new(config_path).await?;
    let app = app::build_app(state::ApiState::new(core));

    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(4000);
    let address = SocketAddr::from(([127, 0, 0, 1], port));

    info!("agent-api listening on http://{address}");
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
