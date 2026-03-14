use std::{collections::HashMap, fs, path::PathBuf, time::Duration};

use async_trait::async_trait;
use rusqlite::Connection;
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
    helpers::{query_json_records, sqlite_error},
    schema,
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
        let connection = self.open_connection()?;
        schema::init_schema(&connection)
    }

    fn seed_defaults(&self, config: &ModelsConfig) -> CoreResult<()> {
        let mut connection = self.open_connection()?;
        schema::seed_defaults(&mut connection, config)
    }

    pub(super) fn open_connection(&self) -> CoreResult<Connection> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| sqlite_error("open sqlite database", error))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| sqlite_error("set sqlite busy timeout", error))?;
        connection
            .execute_batch(
                r#"
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
        let connection = self.open_connection()?;
        let documents = query_json_records::<MemoryDocumentRecord, _>(
            &connection,
            "SELECT data FROM agent_memory_documents",
            [],
            "memory document",
        )?;
        let chunks = query_json_records::<MemoryChunkRecord, _>(
            &connection,
            "SELECT data FROM agent_memory_chunks ORDER BY ordinal ASC",
            [],
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
        limit: usize,
    ) -> CoreResult<Option<Vec<MemorySearchHit>>> {
        let Some(chroma) = &self.chroma else {
            return Ok(None);
        };

        match chroma.query(query, namespace, limit).await {
            Ok(hits) => Ok(Some(hits)),
            Err(error) => {
                warn!(
                    "chroma query failed; memory retrieval is falling back to lexical search: {}",
                    error.message
                );
                Ok(None)
            }
        }
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

    async fn write_run_memory_note(
        &self,
        user_content: &str,
        response: &str,
    ) -> CoreResult<MemoryDocumentRecord> {
        self.write_run_memory_note_db(user_content, response).await
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
