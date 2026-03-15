use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use uuid::Uuid;

use crate::{
    config::ModelsConfig,
    domain::{
        ArtifactKind, ArtifactProducerKind, ArtifactRecord, ArtifactRenderMode,
        CreateMemoryDocumentRequest, CreateSessionRequest, MemoryScope, MemorySearchRequest,
        MessageRole, PlanStepKind, PostMessageRequest, RunStatus, RunStepStatus,
        SessionSkillAvailability, SessionSkillPolicyMode, TaskStatus,
    },
    storage::{AgentStore, SqliteStore},
};

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_sqlite_test() -> std::sync::MutexGuard<'static, ()> {
    test_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    let _guard = lock_sqlite_test();
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
    assert_eq!(
        detail.skill_summary.policy.mode,
        SessionSkillPolicyMode::InheritDefault
    );
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_store_persists_session_skill_policy_and_bindings() {
    let _guard = lock_sqlite_test();
    let _chroma_disabled = EnvVarGuard::set("CHROMA_DISABLED", "1");
    let sqlite_path = unique_sqlite_path();

    let store = SqliteStore::connect(&sqlite_path, &test_models_config())
        .await
        .expect("connect sqlite store");
    let session = store
        .create_session(CreateSessionRequest {
            title: Some("Skill Session".to_string()),
        })
        .await
        .expect("create session");
    let initial = store
        .get_session_skills(session.id)
        .await
        .expect("get session skills");
    let skill_id = initial
        .effective_skills
        .first()
        .expect("seeded effective skill")
        .skill
        .id;

    let updated = store
        .update_session_skill_binding(
            session.id,
            skill_id,
            crate::domain::UpdateSessionSkillBindingRequest {
                availability: SessionSkillAvailability::Pinned,
                order_index: Some(0),
                notes: Some("Keep this visible".to_string()),
            },
        )
        .await
        .expect("pin session skill");
    assert!(updated
        .effective_skills
        .iter()
        .any(|entry| entry.skill.id == skill_id && entry.is_pinned));

    drop(store);

    let reopened = SqliteStore::connect(&sqlite_path, &test_models_config())
        .await
        .expect("reconnect sqlite store");
    let reopened_detail = reopened
        .get_session_skills(session.id)
        .await
        .expect("reopen session skills");
    assert!(reopened_detail
        .effective_skills
        .iter()
        .any(|entry| entry.skill.id == skill_id
            && entry.availability == SessionSkillAvailability::Pinned));
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_store_indexes_and_searches_memory_documents() {
    let _guard = lock_sqlite_test();
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
            memory_scope: Some(MemoryScope::Project),
            owner_session_id: None,
            owner_task_id: None,
            is_pinned: None,
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
            memory_scopes: Some(vec![MemoryScope::Project]),
            owner_session_id: None,
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
    let _guard = lock_sqlite_test();
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
    let plan_detail = store
        .get_task_plan(accepted.run.task_id)
        .await
        .expect("get task plan before reconnect");
    assert_eq!(plan_detail.steps.len(), 2);

    let tool_step = store
        .start_run_step(
            accepted.run.id,
            None,
            PlanStepKind::Tool,
            "Inspect reconnect state".to_string(),
            "Inspect the workspace".to_string(),
        )
        .await
        .expect("start tool step");
    store
        .record_tool_invocation(
            tool_step.id,
            "list".to_string(),
            "local".to_string(),
            serde_json::json!({ "path": "." }),
            serde_json::json!({
                "ok": true,
                "payload": { "entries": [] }
            }),
            true,
            None,
        )
        .await
        .expect("record tool invocation");
    store
        .complete_run_step(tool_step.id, "Listed workspace entries.".to_string())
        .await
        .expect("complete tool step");

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

    store
        .upsert_artifact(ArtifactRecord {
            id: Uuid::new_v4(),
            session_id: session.id,
            task_id: accepted.run.task_id,
            run_id: accepted.run.id,
            path: format!("runs/{}/assistant-response.md", accepted.run.id),
            display_name: "Assistant response".to_string(),
            description: "Reconnect durability artifact".to_string(),
            kind: ArtifactKind::Response,
            media_type: "text/markdown; charset=utf-8".to_string(),
            render_mode: ArtifactRenderMode::Markdown,
            size_bytes: 128,
            producer_kind: Some(ArtifactProducerKind::RunStep),
            producer_ref_id: Some(tool_step.id),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await
        .expect("upsert artifact");

    let document = store
        .create_memory_document(CreateMemoryDocumentRequest {
            title: "Reconnect Memory".to_string(),
            namespace: Some("test".to_string()),
            source: Some("reconnect".to_string()),
            memory_scope: Some(MemoryScope::Project),
            owner_session_id: None,
            owner_task_id: None,
            is_pinned: None,
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

    let tasks = reopened
        .list_tasks(Some(session.id))
        .await
        .expect("list tasks after reconnect");
    let task = tasks
        .iter()
        .find(|task| task.id == accepted.run.task_id)
        .expect("task after reconnect");
    assert!(matches!(task.status, TaskStatus::Completed));
    assert_eq!(task.latest_run_id, Some(accepted.run.id));

    let task_runs = reopened
        .list_task_runs(accepted.run.task_id)
        .await
        .expect("list task runs after reconnect");
    assert_eq!(task_runs.len(), 1);
    assert_eq!(task_runs[0].id, accepted.run.id);

    let reopened_plan = reopened
        .get_task_plan(accepted.run.task_id)
        .await
        .expect("get task plan after reconnect");
    assert_eq!(reopened_plan.plan.id, plan_detail.plan.id);
    assert_eq!(reopened_plan.steps.len(), 2);

    let run_steps = reopened
        .list_run_steps(accepted.run.id)
        .await
        .expect("list run steps after reconnect");
    assert!(run_steps.iter().any(|step| {
        step.id == tool_step.id
            && step.kind == PlanStepKind::Tool
            && matches!(step.status, RunStepStatus::Completed)
    }));

    let tool_invocations = reopened
        .list_tool_invocations(accepted.run.id)
        .await
        .expect("list tool invocations after reconnect");
    assert_eq!(tool_invocations.len(), 1);
    assert_eq!(tool_invocations[0].run_step_id, tool_step.id);
    assert_eq!(tool_invocations[0].tool_name, "list");
    assert_eq!(tool_invocations[0].arguments_json["path"], ".");
    assert_eq!(tool_invocations[0].result_json["ok"], true);

    let artifacts = reopened
        .list_session_artifacts(session.id)
        .await
        .expect("list artifacts after reconnect");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0].path,
        format!("runs/{}/assistant-response.md", accepted.run.id)
    );
    assert!(matches!(artifacts[0].kind, ArtifactKind::Response));
    assert_eq!(
        artifacts[0].producer_kind,
        Some(ArtifactProducerKind::RunStep)
    );
    assert_eq!(artifacts[0].producer_ref_id, Some(tool_step.id));

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
            memory_scopes: Some(vec![MemoryScope::Project]),
            owner_session_id: None,
            limit: Some(5),
        })
        .await
        .expect("search memory after reconnect");
    assert!(search
        .hits
        .iter()
        .any(|hit| hit.document_id == document.id && hit.content.contains(&unique_term)));
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_store_scopes_session_memory_to_its_owner_session() {
    let _guard = lock_sqlite_test();
    let _chroma_disabled = EnvVarGuard::set("CHROMA_DISABLED", "1");

    let store = SqliteStore::connect(unique_sqlite_path(), &test_models_config())
        .await
        .expect("connect sqlite store");
    let session = store
        .create_session(CreateSessionRequest {
            title: Some("Scoped Memory Session".to_string()),
        })
        .await
        .expect("create session");
    let other_session = store
        .create_session(CreateSessionRequest {
            title: Some("Other Session".to_string()),
        })
        .await
        .expect("create other session");

    let document = store
        .create_memory_document(CreateMemoryDocumentRequest {
            title: "Session Note".to_string(),
            namespace: Some("session".to_string()),
            source: Some("unit".to_string()),
            memory_scope: Some(MemoryScope::Session),
            owner_session_id: Some(session.id),
            owner_task_id: None,
            is_pinned: None,
            content: "private session memory token".to_string(),
        })
        .await
        .expect("create session-scoped memory");

    let owner_results = store
        .search_memory(MemorySearchRequest {
            query: "private session memory token".to_string(),
            namespace: Some("session".to_string()),
            memory_scopes: Some(vec![MemoryScope::Session]),
            owner_session_id: Some(session.id),
            limit: Some(3),
        })
        .await
        .expect("search owner session memory");
    assert!(owner_results
        .hits
        .iter()
        .any(|hit| hit.document_id == document.id && hit.owner_session_id == Some(session.id)));

    let other_results = store
        .search_memory(MemorySearchRequest {
            query: "private session memory token".to_string(),
            namespace: Some("session".to_string()),
            memory_scopes: Some(vec![MemoryScope::Session]),
            owner_session_id: Some(other_session.id),
            limit: Some(3),
        })
        .await
        .expect("search other session memory");
    assert!(other_results.hits.is_empty());
}
