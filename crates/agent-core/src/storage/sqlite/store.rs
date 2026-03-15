use std::{collections::HashMap, fs, path::PathBuf};

use async_trait::async_trait;
use diesel::{connection::SimpleConnection, prelude::*, sqlite::SqliteConnection};
use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::ModelsConfig,
    domain::*,
    error::CoreResult,
    memory::{chroma_records_for_document, ChromaClient},
    storage::{AgentStore, RunContext, StoreState},
};

use super::{
    helpers::{load_json_records, sqlite_error},
    schema,
    tables::{agent_memory_chunks, agent_memory_documents},
};

pub struct SqliteStore {
    db_path: PathBuf,
    pub(super) chroma: Option<ChromaClient>,
}

impl SqliteStore {
    pub(crate) async fn connect(
        db_path: impl Into<PathBuf>,
        config: &ModelsConfig,
    ) -> anyhow::Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let store = Self {
            db_path: db_path.clone(),
            chroma: None,
        };
        store.init_schema()?;
        store.seed_defaults(config)?;

        let chroma = match ChromaClient::connect().await {
            Ok(Some(client)) => Some(client),
            Ok(None) => None,
            Err(error) => {
                warn!(
                    "chroma disabled; memory retrieval will fall back to lexical search: {error}"
                );
                None
            }
        };

        let store = Self { db_path, chroma };
        if let Some(chroma) = &store.chroma {
            if let Err(error) = store.reindex_chroma(chroma).await {
                warn!(
                    "failed to sync sqlite memory into chroma at startup: {}",
                    error.message
                );
            }
        }

        Ok(store)
    }

    fn init_schema(&self) -> CoreResult<()> {
        let mut connection = self.open_connection()?;
        schema::init_schema(&mut connection)
    }

    fn seed_defaults(&self, config: &ModelsConfig) -> CoreResult<()> {
        let mut connection = self.open_connection()?;
        schema::seed_defaults(&mut connection, config)
    }

    pub(super) fn open_connection(&self) -> CoreResult<SqliteConnection> {
        let db_path = self.db_path.to_string_lossy().to_string();
        let mut connection = SqliteConnection::establish(&db_path)
            .map_err(|error| sqlite_error("open sqlite database", error))?;
        connection
            .batch_execute(
                r#"
                PRAGMA busy_timeout = 5000;
                PRAGMA foreign_keys = ON;
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                "#,
            )
            .map_err(|error| sqlite_error("configure sqlite connection", error))?;
        Ok(connection)
    }

    async fn reindex_chroma(&self, chroma: &ChromaClient) -> CoreResult<()> {
        let memory_state = self.load_memory_state()?;
        let mut records = Vec::new();

        for document in memory_state.memory_documents.values() {
            records.extend(chroma_records_for_document(document));
        }

        chroma.reset_collection().await?;
        if !records.is_empty() {
            chroma.upsert_records(records).await?;
        }
        Ok(())
    }

    pub(super) fn load_memory_state(&self) -> CoreResult<StoreState> {
        let mut connection = self.open_connection()?;
        let documents = load_json_records::<MemoryDocumentRecord, _>(
            &mut connection,
            agent_memory_documents::table.select(agent_memory_documents::data),
            "memory document",
        )?;
        let chunks = load_json_records::<MemoryChunkRecord, _>(
            &mut connection,
            agent_memory_chunks::table
                .order(agent_memory_chunks::ordinal.asc())
                .select(agent_memory_chunks::data),
            "memory chunk",
        )?;

        Ok(StoreState {
            memory_documents: documents
                .into_iter()
                .map(|document| (document.id, document))
                .collect::<HashMap<Uuid, MemoryDocumentRecord>>(),
            memory_chunks: chunks,
            ..StoreState::default()
        })
    }

    pub(super) async fn search_memory_semantic(
        &self,
        query: &str,
        namespace: Option<&str>,
        memory_scopes: Option<&[MemoryScope]>,
        owner_session_id: Option<Uuid>,
        limit: usize,
    ) -> CoreResult<Option<Vec<MemorySearchHit>>> {
        let Some(chroma) = &self.chroma else {
            return Ok(None);
        };

        let requested_scopes = memory_scopes
            .map(|scopes| scopes.to_vec())
            .unwrap_or_else(|| {
                vec![
                    MemoryScope::Session,
                    MemoryScope::Project,
                    MemoryScope::Global,
                ]
            });

        let mut hit_sets = Vec::new();
        for scope in requested_scopes {
            let scoped_owner_session = matches!(scope, MemoryScope::Session)
                .then_some(owner_session_id)
                .flatten();
            let hits = match chroma
                .query(query, namespace, Some(&scope), scoped_owner_session, limit)
                .await
            {
                Ok(hits) => hits,
                Err(error) => {
                    warn!(
                        "chroma query failed; memory retrieval is falling back to lexical search: {}",
                        error.message
                    );
                    return Ok(None);
                }
            };
            hit_sets.push(hits);
        }

        Ok(Some(crate::memory::merge_memory_hits(hit_sets, limit)))
    }
}

#[async_trait]
impl AgentStore for SqliteStore {
    async fn list_sessions(&self) -> CoreResult<Vec<SessionRecord>> {
        self.list_sessions_db().await
    }

    async fn create_session(&self, payload: CreateSessionRequest) -> CoreResult<SessionRecord> {
        self.create_session_db(payload).await
    }

    async fn get_session(&self, session_id: Uuid) -> CoreResult<SessionDetail> {
        self.get_session_db(session_id).await
    }

    async fn update_session(
        &self,
        session_id: Uuid,
        payload: UpdateSessionRequest,
    ) -> CoreResult<SessionRecord> {
        self.update_session_db(session_id, payload).await
    }

    async fn delete_session(&self, session_id: Uuid) -> CoreResult<()> {
        self.delete_session_db(session_id).await
    }

    async fn list_messages(&self, session_id: Uuid) -> CoreResult<Vec<MessageRecord>> {
        self.list_messages_db(session_id).await
    }

    async fn enqueue_user_message(
        &self,
        session_id: Uuid,
        payload: PostMessageRequest,
    ) -> CoreResult<RunAccepted> {
        self.enqueue_user_message_db(session_id, payload).await
    }

    async fn get_active_run(&self, session_id: Uuid) -> CoreResult<Option<RunRecord>> {
        self.get_active_run_db(session_id).await
    }

    async fn get_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        self.get_run_db(run_id).await
    }

    async fn cancel_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        self.cancel_run_db(run_id).await
    }

    async fn fail_run(&self, run_id: Uuid, message: String) -> CoreResult<RunRecord> {
        self.fail_run_db(run_id, message).await
    }

    async fn run_is_active(&self, run_id: Uuid) -> bool {
        self.run_is_active_db(run_id).await
    }

    async fn list_run_events(
        &self,
        run_id: Uuid,
        after_sequence: Option<u64>,
    ) -> CoreResult<Vec<RunEventEnvelope>> {
        self.list_run_events_db(run_id, after_sequence).await
    }

    async fn append_run_event(&self, event: RunEventEnvelope) -> CoreResult<()> {
        self.append_run_event_db(event).await
    }

    async fn set_run_selection(
        &self,
        run_id: Uuid,
        provider_name: String,
        model_name: String,
    ) -> CoreResult<()> {
        self.set_run_selection_db(run_id, provider_name, model_name)
            .await
    }

    async fn build_run_context(
        &self,
        session_id: Uuid,
        user_content: &str,
    ) -> CoreResult<RunContext> {
        self.build_run_context_db(session_id, user_content).await
    }

    async fn append_assistant_message_and_complete_run(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        response: String,
    ) -> CoreResult<MessageRecord> {
        self.append_assistant_message_and_complete_run_db(session_id, run_id, response)
            .await
    }

    async fn list_tasks(&self, session_id: Option<Uuid>) -> CoreResult<Vec<TaskRecord>> {
        self.list_tasks_db(session_id).await
    }

    async fn get_task(&self, task_id: Uuid) -> CoreResult<TaskRecord> {
        self.get_task_db(task_id).await
    }

    async fn get_task_plan(&self, task_id: Uuid) -> CoreResult<PlanDetail> {
        self.get_task_plan_db(task_id).await
    }

    async fn list_task_runs(&self, task_id: Uuid) -> CoreResult<Vec<RunRecord>> {
        self.list_task_runs_db(task_id).await
    }

    async fn list_session_artifacts(&self, session_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        self.list_session_artifacts_db(session_id).await
    }

    async fn list_task_artifacts(&self, task_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        self.list_task_artifacts_db(task_id).await
    }

    async fn list_run_artifacts(&self, run_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        self.list_run_artifacts_db(run_id).await
    }

    async fn upsert_artifact(&self, artifact: ArtifactRecord) -> CoreResult<ArtifactRecord> {
        self.upsert_artifact_db(artifact).await
    }

    async fn list_run_steps(&self, run_id: Uuid) -> CoreResult<Vec<RunStepRecord>> {
        self.list_run_steps_db(run_id).await
    }

    async fn start_run_step(
        &self,
        run_id: Uuid,
        plan_step_id: Option<Uuid>,
        kind: PlanStepKind,
        title: String,
        input_summary: String,
    ) -> CoreResult<RunStepRecord> {
        self.start_run_step_db(run_id, plan_step_id, kind, title, input_summary)
            .await
    }

    async fn complete_run_step(
        &self,
        run_step_id: Uuid,
        output_summary: String,
    ) -> CoreResult<RunStepRecord> {
        self.complete_run_step_db(run_step_id, output_summary).await
    }

    async fn fail_run_step(&self, run_step_id: Uuid, error: String) -> CoreResult<RunStepRecord> {
        self.fail_run_step_db(run_step_id, error).await
    }

    async fn list_tool_invocations(&self, run_id: Uuid) -> CoreResult<Vec<ToolInvocationRecord>> {
        self.list_tool_invocations_db(run_id).await
    }

    async fn record_tool_invocation(
        &self,
        run_step_id: Uuid,
        tool_name: String,
        tool_source: String,
        arguments_json: Value,
        result_json: Value,
        ok: bool,
        error: Option<String>,
    ) -> CoreResult<ToolInvocationRecord> {
        self.record_tool_invocation_db(
            run_step_id,
            tool_name,
            tool_source,
            arguments_json,
            result_json,
            ok,
            error,
        )
        .await
    }

    async fn write_run_memory_note(
        &self,
        session_id: Uuid,
        user_content: &str,
        response: &str,
    ) -> CoreResult<MemoryDocumentRecord> {
        self.write_run_memory_note_db(session_id, user_content, response)
            .await
    }

    async fn list_skills(&self) -> CoreResult<Vec<SkillRecord>> {
        self.list_skills_db().await
    }

    async fn create_skill(&self, payload: CreateSkillRequest) -> CoreResult<SkillRecord> {
        self.create_skill_db(payload).await
    }

    async fn update_skill(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        self.update_skill_db(skill_id, payload).await
    }

    async fn list_skill_presets(&self) -> CoreResult<Vec<SkillPreset>> {
        self.list_skill_presets_db().await
    }

    async fn get_session_skills(&self, session_id: Uuid) -> CoreResult<SessionSkillsDetail> {
        self.get_session_skills_db(session_id).await
    }

    async fn replace_session_skills(
        &self,
        session_id: Uuid,
        detail: SessionSkillsDetail,
    ) -> CoreResult<SessionSkillsDetail> {
        self.replace_session_skills_db(session_id, detail).await
    }

    async fn update_session_skill_binding(
        &self,
        session_id: Uuid,
        skill_id: Uuid,
        payload: UpdateSessionSkillBindingRequest,
    ) -> CoreResult<SessionSkillsDetail> {
        self.update_session_skill_binding_db(session_id, skill_id, payload)
            .await
    }

    async fn apply_session_skill_preset(
        &self,
        session_id: Uuid,
        preset_id: String,
    ) -> CoreResult<SessionSkillsDetail> {
        self.apply_session_skill_preset_db(session_id, preset_id)
            .await
    }

    async fn list_subagents(&self) -> CoreResult<Vec<SubagentRecord>> {
        self.list_subagents_db().await
    }

    async fn create_subagent(&self, payload: CreateSubagentRequest) -> CoreResult<SubagentRecord> {
        self.create_subagent_db(payload).await
    }

    async fn get_subagent(&self, subagent_id: Uuid) -> CoreResult<SubagentRecord> {
        self.get_subagent_db(subagent_id).await
    }

    async fn update_subagent(
        &self,
        subagent_id: Uuid,
        payload: UpdateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        self.update_subagent_db(subagent_id, payload).await
    }

    async fn list_providers(&self) -> CoreResult<Vec<ProviderAccountRecord>> {
        self.list_providers_db().await
    }

    async fn create_provider(
        &self,
        payload: CreateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        self.create_provider_db(payload).await
    }

    async fn get_provider(&self, provider_id: Uuid) -> CoreResult<ProviderAccountRecord> {
        self.get_provider_db(provider_id).await
    }

    async fn update_provider(
        &self,
        provider_id: Uuid,
        payload: UpdateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        self.update_provider_db(provider_id, payload).await
    }

    async fn list_provider_models(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<Vec<ProviderModelRecord>> {
        self.list_provider_models_db(provider_id).await
    }

    async fn replace_provider_models(
        &self,
        provider_id: Uuid,
        base_url: Option<String>,
        models: Vec<ProviderModelRecord>,
    ) -> CoreResult<ProviderAccountRecord> {
        self.replace_provider_models_db(provider_id, base_url, models)
            .await
    }

    async fn list_memory_documents(&self) -> CoreResult<Vec<MemoryDocumentRecord>> {
        self.list_memory_documents_db().await
    }

    async fn create_memory_document(
        &self,
        payload: CreateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        self.create_memory_document_db(payload).await
    }

    async fn update_memory_document(
        &self,
        document_id: Uuid,
        payload: UpdateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        self.update_memory_document_db(document_id, payload).await
    }

    async fn delete_memory_document(&self, document_id: Uuid) -> CoreResult<()> {
        self.delete_memory_document_db(document_id).await
    }

    async fn get_memory_document(&self, document_id: Uuid) -> CoreResult<MemoryDocumentDetail> {
        self.get_memory_document_db(document_id).await
    }

    async fn search_memory(&self, payload: MemorySearchRequest) -> CoreResult<MemorySearchResult> {
        self.search_memory_db(payload).await
    }

    async fn reindex_memory(&self) -> CoreResult<ReindexResult> {
        self.reindex_memory_db().await
    }

    async fn list_mcp_servers(&self) -> CoreResult<Vec<McpServerRecord>> {
        self.list_mcp_servers_db().await
    }

    async fn create_mcp_server(
        &self,
        payload: CreateMcpServerRequest,
    ) -> CoreResult<McpServerRecord> {
        self.create_mcp_server_db(payload).await
    }

    async fn get_mcp_server(&self, server_id: Uuid) -> CoreResult<McpServerRecord> {
        self.get_mcp_server_db(server_id).await
    }

    async fn test_mcp_server(&self, server_id: Uuid) -> CoreResult<TestResult> {
        self.test_mcp_server_db(server_id).await
    }

    async fn get_mcp_capabilities(&self, server_id: Uuid) -> CoreResult<CapabilityEnvelope> {
        self.get_mcp_capabilities_db(server_id).await
    }
}
