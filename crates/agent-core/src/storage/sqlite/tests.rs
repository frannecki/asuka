use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use uuid::Uuid;

use crate::{
    config::ModelsConfig,
    domain::{
        CreateMemoryDocumentRequest, CreateSessionRequest, MemorySearchRequest, MessageRole,
        PostMessageRequest, RunStatus,
    },
    storage::{AgentStore, SqliteStore},
};

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn test_models_config() -> ModelsConfig {
    toml::from_str(
        r#"
[[providers]]
provider_type = "openRouter"
display_name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
default_model = "demo-model"

[[providers.models]]
name = "demo-model"
context_window = 8192
supports_tools = false
supports_embeddings = false
capabilities = ["chat"]
"#,
    )
    .expect("parse test models config")
}

fn unique_sqlite_path() -> PathBuf {
    std::env::temp_dir().join(format!("asuka-test-{}.sqlite3", Uuid::new_v4()))
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(previous) => std::env::set_var(self.key, previous),
            None => std::env::remove_var(self.key),
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_store_persists_sessions() {
    let _guard = test_lock().lock().expect("lock sqlite test");
    let _chroma_disabled = EnvVarGuard::set("CHROMA_DISABLED", "1");

    let store = SqliteStore::connect(unique_sqlite_path(), &test_models_config())
        .await
        .expect("connect sqlite store");

    let created = store
        .create_session(CreateSessionRequest {
            title: Some("Test Session".to_string()),
        })
        .await
        .expect("create session");

    let sessions = store.list_sessions().await.expect("list sessions");
    assert!(sessions.iter().any(|session| session.id == created.id));

    let detail = store.get_session(created.id).await.expect("get session");
    assert_eq!(detail.session.title, "Test Session");
    assert!(detail.messages.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_store_indexes_and_searches_memory_documents() {
    let _guard = test_lock().lock().expect("lock sqlite test");
    let _chroma_disabled = EnvVarGuard::set("CHROMA_DISABLED", "1");

    let store = SqliteStore::connect(unique_sqlite_path(), &test_models_config())
        .await
        .expect("connect sqlite store");
    let unique_term = format!("token-{}", Uuid::new_v4().simple());

    let document = store
        .create_memory_document(CreateMemoryDocumentRequest {
            title: "Unique Memory".to_string(),
            namespace: Some("test".to_string()),
            source: Some("unit".to_string()),
            content: format!("This document contains {unique_term} for retrieval."),
        })
        .await
        .expect("create memory document");

    assert!(document.chunk_count >= 1);

    let detail = store
        .get_memory_document(document.id)
        .await
        .expect("get memory document");
    assert_eq!(detail.document.title, "Unique Memory");
    assert!(!detail.chunks.is_empty());

    let results = store
        .search_memory(MemorySearchRequest {
            query: unique_term.clone(),
            namespace: Some("test".to_string()),
            limit: Some(3),
        })
        .await
        .expect("search memory");

    assert!(results
        .hits
        .iter()
        .any(|hit| hit.document_id == document.id && hit.content.contains(&unique_term)));

    let reindex = store.reindex_memory().await.expect("reindex memory");
    assert!(reindex.documents >= 1);
    assert!(reindex.chunks >= detail.chunks.len());
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_store_persists_user_state_across_reconnect_without_reseeding_duplicates() {
    let _guard = test_lock().lock().expect("lock sqlite test");
    let _chroma_disabled = EnvVarGuard::set("CHROMA_DISABLED", "1");
    let sqlite_path = unique_sqlite_path();
    let unique_term = format!("reconnect-{}", Uuid::new_v4().simple());

    let store = SqliteStore::connect(&sqlite_path, &test_models_config())
        .await
        .expect("connect sqlite store");

    let seeded_provider_count = store.list_providers().await.expect("list providers").len();
    let seeded_skill_count = store.list_skills().await.expect("list skills").len();
    let seeded_subagent_count = store.list_subagents().await.expect("list subagents").len();
    let seeded_mcp_count = store
        .list_mcp_servers()
        .await
        .expect("list mcp servers")
        .len();

    let session = store
        .create_session(CreateSessionRequest {
            title: Some("Reconnect Session".to_string()),
        })
        .await
        .expect("create session");

    let accepted = store
        .enqueue_user_message(
            session.id,
            PostMessageRequest {
                content: format!("Please remember {unique_term} across reconnect."),
            },
        )
        .await
        .expect("enqueue user message");

    store
        .set_run_selection(
            accepted.run.id,
            "OpenRouter".to_string(),
            "demo-model".to_string(),
        )
        .await
        .expect("set run selection");

    let assistant = store
        .append_assistant_message_and_complete_run(
            session.id,
            accepted.run.id,
            format!("Persisted assistant reply mentioning {unique_term}."),
        )
        .await
        .expect("append assistant message");

    let document = store
        .create_memory_document(CreateMemoryDocumentRequest {
            title: "Reconnect Memory".to_string(),
            namespace: Some("test".to_string()),
            source: Some("reconnect".to_string()),
            content: format!("Local durability should preserve {unique_term} after reconnect."),
        })
        .await
        .expect("create memory document");

    drop(store);

    let reopened = SqliteStore::connect(&sqlite_path, &test_models_config())
        .await
        .expect("reconnect sqlite store");

    assert_eq!(
        reopened
            .list_providers()
            .await
            .expect("list providers after reconnect")
            .len(),
        seeded_provider_count
    );
    assert_eq!(
        reopened
            .list_skills()
            .await
            .expect("list skills after reconnect")
            .len(),
        seeded_skill_count
    );
    assert_eq!(
        reopened
            .list_subagents()
            .await
            .expect("list subagents after reconnect")
            .len(),
        seeded_subagent_count
    );
    assert_eq!(
        reopened
            .list_mcp_servers()
            .await
            .expect("list mcp servers after reconnect")
            .len(),
        seeded_mcp_count
    );

    let detail = reopened
        .get_session(session.id)
        .await
        .expect("get session after reconnect");
    assert_eq!(detail.session.title, "Reconnect Session");
    assert_eq!(detail.messages.len(), 2);
    assert!(matches!(detail.messages[0].role, MessageRole::User));
    assert!(detail.messages[0].content.contains(&unique_term));
    assert!(matches!(detail.messages[1].role, MessageRole::Assistant));
    assert_eq!(detail.messages[1].id, assistant.id);
    assert!(detail.messages[1].content.contains(&unique_term));

    let run = reopened
        .get_run(accepted.run.id)
        .await
        .expect("get run after reconnect");
    assert!(matches!(run.status, RunStatus::Completed));
    assert_eq!(run.selected_provider.as_deref(), Some("OpenRouter"));
    assert_eq!(run.selected_model.as_deref(), Some("demo-model"));

    let memory_detail = reopened
        .get_memory_document(document.id)
        .await
        .expect("get memory document after reconnect");
    assert_eq!(memory_detail.document.title, "Reconnect Memory");
    assert!(!memory_detail.chunks.is_empty());

    let search = reopened
        .search_memory(MemorySearchRequest {
            query: unique_term.clone(),
            namespace: Some("test".to_string()),
            limit: Some(5),
        })
        .await
        .expect("search memory after reconnect");
    assert!(search
        .hits
        .iter()
        .any(|hit| hit.document_id == document.id && hit.content.contains(&unique_term)));
}
