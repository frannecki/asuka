use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc},
};

use serde_json::Value;
use tokio::sync::broadcast;
use tracing::warn;

use crate::{
    config::load_models_config,
    core::docs,
    domain::RunEventEnvelope,
    providers::openrouter::client::{HyperOpenRouterTransport, OpenRouterTransport},
    storage::{AgentStore, InMemoryStore, SqliteStore},
    tools::ToolRegistry,
};

#[derive(Clone)]
pub struct AgentCore {
    pub(crate) store: Arc<dyn AgentStore>,
    pub(crate) event_tx: broadcast::Sender<RunEventEnvelope>,
    pub(crate) event_sequence: Arc<AtomicU64>,
    pub(crate) config_path: Arc<PathBuf>,
    pub(crate) openrouter_transport: Arc<dyn OpenRouterTransport>,
    pub(crate) tool_registry: Arc<ToolRegistry>,
}

impl AgentCore {
    pub async fn new(config_path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let config_path = config_path.into();
        let config = load_models_config(&config_path)?;
        let store_kind = std::env::var("AGENT_STORE")
            .ok()
            .filter(|value| !value.is_empty());
        let store: Arc<dyn AgentStore> = match store_kind.as_deref() {
            Some("memory") => Arc::new(InMemoryStore::seeded(&config)),
            Some("sqlite") => {
                let sqlite_path =
                    std::env::var("SQLITE_PATH").unwrap_or_else(|_| "./data/asuka.sqlite3".into());
                Arc::new(SqliteStore::connect(sqlite_path, &config).await?)
            }
            Some(other) => {
                return Err(anyhow::anyhow!(
                    "unsupported AGENT_STORE value: {other}. expected 'memory' or 'sqlite'"
                ))
            }
            None => {
                let sqlite_path =
                    std::env::var("SQLITE_PATH").unwrap_or_else(|_| "./data/asuka.sqlite3".into());
                Arc::new(SqliteStore::connect(sqlite_path, &config).await?)
            }
        };
        Self::with_store(config_path, store)
    }

    pub fn with_store(
        config_path: impl Into<PathBuf>,
        store: Arc<dyn AgentStore>,
    ) -> anyhow::Result<Self> {
        Self::with_store_and_openrouter_transport(
            config_path,
            store,
            Arc::new(HyperOpenRouterTransport::new()),
        )
    }

    pub(crate) fn with_store_and_openrouter_transport(
        config_path: impl Into<PathBuf>,
        store: Arc<dyn AgentStore>,
        openrouter_transport: Arc<dyn OpenRouterTransport>,
    ) -> anyhow::Result<Self> {
        let config_path = config_path.into();
        load_models_config(&config_path)?;
        let (event_tx, _) = broadcast::channel(512);
        let workspace_root = std::env::var("ASUKA_WORKSPACE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        Ok(Self {
            store,
            event_tx,
            event_sequence: Arc::new(AtomicU64::new(1)),
            config_path: Arc::new(config_path),
            openrouter_transport,
            tool_registry: Arc::new(ToolRegistry::new(workspace_root)),
        })
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<RunEventEnvelope> {
        self.event_tx.subscribe()
    }

    pub fn root_docs(&self) -> Value {
        docs::root_docs()
    }
}

impl Drop for AgentCore {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }

        if Arc::strong_count(&self.store) == 1 {
            warn!("agent-core is shutting down");
        }
    }
}
