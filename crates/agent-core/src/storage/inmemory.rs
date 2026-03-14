use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    config::ModelsConfig,
    domain::*,
    error::{CoreError, CoreResult},
    memory::{chunk_memory_document, search_memory_hits, summarize_text, MemoryCorpus},
    storage::{AgentStore, RunContext, StoreState},
};

pub struct InMemoryStore {
    state: RwLock<StoreState>,
}

impl InMemoryStore {
    pub(crate) fn seeded(config: &ModelsConfig) -> Self {
        Self {
            state: RwLock::new(StoreState::seeded(config)),
        }
    }
}

#[async_trait]
impl AgentStore for InMemoryStore {
    async fn list_sessions(&self) -> CoreResult<Vec<SessionRecord>> {
        let state = self.state.read().await;
        let mut sessions = state.sessions.values().cloned().collect::<Vec<_>>();
        sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
        Ok(sessions)
    }

    async fn create_session(&self, payload: CreateSessionRequest) -> CoreResult<SessionRecord> {
        let session = SessionRecord {
            id: Uuid::new_v4(),
            title: payload
                .title
                .unwrap_or_else(|| format!("Session {}", Utc::now().format("%H:%M:%S"))),
            status: SessionStatus::Active,
            root_agent_id: "default-root-agent".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_run_at: None,
            summary: "New session".to_string(),
        };

        let mut state = self.state.write().await;
        state.messages.insert(session.id, Vec::new());
        state.sessions.insert(session.id, session.clone());
        Ok(session)
    }

    async fn get_session(&self, session_id: Uuid) -> CoreResult<SessionDetail> {
        let state = self.state.read().await;
        let session = state
            .sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("session"))?;
        let messages = state.messages.get(&session_id).cloned().unwrap_or_default();
        Ok(SessionDetail { session, messages })
    }

    async fn update_session(
        &self,
        session_id: Uuid,
        payload: UpdateSessionRequest,
    ) -> CoreResult<SessionRecord> {
        let mut state = self.state.write().await;
        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| CoreError::not_found("session"))?;
        if let Some(title) = payload.title {
            session.title = title;
        }
        if let Some(status) = payload.status {
            session.status = status;
        }
        session.updated_at = Utc::now();
        Ok(session.clone())
    }

    async fn delete_session(&self, session_id: Uuid) -> CoreResult<()> {
        let mut state = self.state.write().await;
        if state.sessions.remove(&session_id).is_none() {
            return Err(CoreError::not_found("session"));
        }
        state.messages.remove(&session_id);
        Ok(())
    }

    async fn list_messages(&self, session_id: Uuid) -> CoreResult<Vec<MessageRecord>> {
        let state = self.state.read().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }
        Ok(state.messages.get(&session_id).cloned().unwrap_or_default())
    }

    async fn enqueue_user_message(
        &self,
        session_id: Uuid,
        payload: PostMessageRequest,
    ) -> CoreResult<RunAccepted> {
        if payload.content.trim().is_empty() {
            return Err(CoreError::bad_request("message content cannot be empty"));
        }

        let run = RunRecord {
            id: Uuid::new_v4(),
            session_id,
            trigger_type: "userMessage".to_string(),
            status: RunStatus::Running,
            selected_provider: None,
            selected_model: None,
            started_at: Utc::now(),
            finished_at: None,
            error: None,
        };

        let user_message = MessageRecord {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::User,
            content: payload.content.trim().to_string(),
            created_at: Utc::now(),
            run_id: Some(run.id),
        };

        let mut state = self.state.write().await;
        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| CoreError::not_found("session"))?;
        session.updated_at = Utc::now();
        session.last_run_at = Some(Utc::now());
        state.runs.insert(run.id, run.clone());
        state
            .messages
            .entry(session_id)
            .or_default()
            .push(user_message.clone());

        Ok(RunAccepted { run, user_message })
    }

    async fn get_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let state = self.state.read().await;
        state
            .runs
            .get(&run_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("run"))
    }

    async fn cancel_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let mut state = self.state.write().await;
        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.status = RunStatus::Cancelled;
        run.finished_at = Some(Utc::now());
        Ok(run.clone())
    }

    async fn fail_run(&self, run_id: Uuid, message: String) -> CoreResult<RunRecord> {
        let mut state = self.state.write().await;
        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.status = RunStatus::Failed;
        run.finished_at = Some(Utc::now());
        run.error = Some(message);
        Ok(run.clone())
    }

    async fn run_is_active(&self, run_id: Uuid) -> bool {
        let state = self.state.read().await;
        state
            .runs
            .get(&run_id)
            .map(|run| matches!(run.status, RunStatus::Running))
            .unwrap_or(false)
    }

    async fn set_run_selection(
        &self,
        run_id: Uuid,
        provider_name: String,
        model_name: String,
    ) -> CoreResult<()> {
        let mut state = self.state.write().await;
        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.selected_provider = Some(provider_name);
        run.selected_model = Some(model_name);
        Ok(())
    }

    async fn build_run_context(
        &self,
        session_id: Uuid,
        user_content: &str,
    ) -> CoreResult<RunContext> {
        let state = self.state.read().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        let recent_messages = state
            .messages
            .get(&session_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();

        let mut providers = state.providers.values().cloned().collect::<Vec<_>>();
        providers.sort_by(|left, right| left.display_name.cmp(&right.display_name));

        Ok(RunContext {
            providers,
            recent_messages,
            memory_hits: search_memory_hits(
                MemoryCorpus {
                    documents: &state.memory_documents,
                    chunks: &state.memory_chunks,
                },
                user_content,
                None,
                3,
            ),
        })
    }

    async fn append_assistant_message_and_complete_run(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        response: String,
    ) -> CoreResult<MessageRecord> {
        let assistant_message = MessageRecord {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::Assistant,
            content: response,
            created_at: Utc::now(),
            run_id: Some(run_id),
        };

        let mut state = self.state.write().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        let run = state
            .runs
            .get(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        if !matches!(run.status, RunStatus::Running) {
            return Err(CoreError::conflict("run is no longer active"));
        }

        state
            .messages
            .entry(session_id)
            .or_default()
            .push(assistant_message.clone());

        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.status = RunStatus::Completed;
        run.finished_at = Some(Utc::now());
        run.error = None;

        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.updated_at = Utc::now();
            session.summary = "Last run completed through the agent-core runtime.".to_string();
        }

        Ok(assistant_message)
    }

    async fn write_run_memory_note(
        &self,
        user_content: &str,
        response: &str,
    ) -> CoreResult<MemoryDocumentRecord> {
        let memory_document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: format!("Run note {}", Utc::now().format("%Y-%m-%d %H:%M:%S")),
            namespace: "session".to_string(),
            source: "run-summary".to_string(),
            content: format!("User request: {user_content}\n\nAssistant response: {response}"),
            summary: summarize_text(response, 24),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut memory_document = memory_document;
        let chunks = chunk_memory_document(&memory_document);
        memory_document.chunk_count = chunks.len();

        let mut state = self.state.write().await;
        state.memory_chunks.extend(chunks);
        state
            .memory_documents
            .insert(memory_document.id, memory_document.clone());
        Ok(memory_document)
    }

    async fn list_skills(&self) -> CoreResult<Vec<SkillRecord>> {
        let state = self.state.read().await;
        let mut skills = state.skills.values().cloned().collect::<Vec<_>>();
        skills.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(skills)
    }

    async fn create_skill(&self, payload: CreateSkillRequest) -> CoreResult<SkillRecord> {
        let skill = SkillRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            description: payload.description,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.write().await;
        state.skills.insert(skill.id, skill.clone());
        Ok(skill)
    }

    async fn update_skill(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        let mut state = self.state.write().await;
        let skill = state
            .skills
            .get_mut(&skill_id)
            .ok_or_else(|| CoreError::not_found("skill"))?;
        if let Some(description) = payload.description {
            skill.description = description;
        }
        if let Some(status) = payload.status {
            skill.status = status;
        }
        skill.updated_at = Utc::now();
        Ok(skill.clone())
    }

    async fn list_subagents(&self) -> CoreResult<Vec<SubagentRecord>> {
        let state = self.state.read().await;
        let mut subagents = state.subagents.values().cloned().collect::<Vec<_>>();
        subagents.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(subagents)
    }

    async fn create_subagent(&self, payload: CreateSubagentRequest) -> CoreResult<SubagentRecord> {
        let subagent = SubagentRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            description: payload.description,
            scope: payload.scope,
            max_steps: payload.max_steps,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.write().await;
        state.subagents.insert(subagent.id, subagent.clone());
        Ok(subagent)
    }

    async fn get_subagent(&self, subagent_id: Uuid) -> CoreResult<SubagentRecord> {
        let state = self.state.read().await;
        state
            .subagents
            .get(&subagent_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("subagent"))
    }

    async fn update_subagent(
        &self,
        subagent_id: Uuid,
        payload: UpdateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        let mut state = self.state.write().await;
        let subagent = state
            .subagents
            .get_mut(&subagent_id)
            .ok_or_else(|| CoreError::not_found("subagent"))?;
        if let Some(description) = payload.description {
            subagent.description = description;
        }
        if let Some(scope) = payload.scope {
            subagent.scope = scope;
        }
        if let Some(max_steps) = payload.max_steps {
            subagent.max_steps = max_steps;
        }
        if let Some(status) = payload.status {
            subagent.status = status;
        }
        subagent.updated_at = Utc::now();
        Ok(subagent.clone())
    }

    async fn list_providers(&self) -> CoreResult<Vec<ProviderAccountRecord>> {
        let state = self.state.read().await;
        let mut providers = state.providers.values().cloned().collect::<Vec<_>>();
        providers.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Ok(providers)
    }

    async fn create_provider(
        &self,
        payload: CreateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        let provider = ProviderAccountRecord {
            id: Uuid::new_v4(),
            provider_type: payload.provider_type,
            display_name: payload.display_name,
            base_url: payload.base_url,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            models: Vec::new(),
        };

        let mut state = self.state.write().await;
        state.providers.insert(provider.id, provider.clone());
        Ok(provider)
    }

    async fn get_provider(&self, provider_id: Uuid) -> CoreResult<ProviderAccountRecord> {
        let state = self.state.read().await;
        state
            .providers
            .get(&provider_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("provider"))
    }

    async fn update_provider(
        &self,
        provider_id: Uuid,
        payload: UpdateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        let mut state = self.state.write().await;
        let provider = state
            .providers
            .get_mut(&provider_id)
            .ok_or_else(|| CoreError::not_found("provider"))?;
        if let Some(display_name) = payload.display_name {
            provider.display_name = display_name;
        }
        if let Some(base_url) = payload.base_url {
            provider.base_url = Some(base_url);
        }
        if let Some(status) = payload.status {
            provider.status = status;
        }
        provider.updated_at = Utc::now();
        Ok(provider.clone())
    }

    async fn list_provider_models(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<Vec<ProviderModelRecord>> {
        let state = self.state.read().await;
        let provider = state
            .providers
            .get(&provider_id)
            .ok_or_else(|| CoreError::not_found("provider"))?;
        Ok(provider.models.clone())
    }

    async fn replace_provider_models(
        &self,
        provider_id: Uuid,
        base_url: Option<String>,
        models: Vec<ProviderModelRecord>,
    ) -> CoreResult<ProviderAccountRecord> {
        let mut state = self.state.write().await;
        let provider = state
            .providers
            .get_mut(&provider_id)
            .ok_or_else(|| CoreError::not_found("provider"))?;
        provider.base_url = base_url;
        provider.models = models;
        provider.updated_at = Utc::now();
        Ok(provider.clone())
    }

    async fn list_memory_documents(&self) -> CoreResult<Vec<MemoryDocumentRecord>> {
        let state = self.state.read().await;
        let mut documents = state.memory_documents.values().cloned().collect::<Vec<_>>();
        documents.sort_by_key(|document| std::cmp::Reverse(document.updated_at));
        Ok(documents)
    }

    async fn create_memory_document(
        &self,
        payload: CreateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        if payload.title.trim().is_empty() || payload.content.trim().is_empty() {
            return Err(CoreError::bad_request(
                "memory documents require both title and content",
            ));
        }

        let document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: payload.title.trim().to_string(),
            namespace: payload
                .namespace
                .unwrap_or_else(|| "global".to_string())
                .trim()
                .to_string(),
            source: payload
                .source
                .unwrap_or_else(|| "manual".to_string())
                .trim()
                .to_string(),
            content: payload.content.trim().to_string(),
            summary: summarize_text(payload.content.trim(), 20),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut document = document;
        let chunks = chunk_memory_document(&document);
        document.chunk_count = chunks.len();

        let mut state = self.state.write().await;
        state.memory_chunks.extend(chunks);
        state.memory_documents.insert(document.id, document.clone());
        Ok(document)
    }

    async fn get_memory_document(&self, document_id: Uuid) -> CoreResult<MemoryDocumentDetail> {
        let state = self.state.read().await;
        let document = state
            .memory_documents
            .get(&document_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("memory document"))?;
        let chunks = state
            .memory_chunks
            .iter()
            .filter(|chunk| chunk.document_id == document_id)
            .cloned()
            .collect::<Vec<_>>();
        Ok(MemoryDocumentDetail { document, chunks })
    }

    async fn search_memory(&self, payload: MemorySearchRequest) -> CoreResult<MemorySearchResult> {
        if payload.query.trim().is_empty() {
            return Err(CoreError::bad_request(
                "memory search query cannot be empty",
            ));
        }

        let state = self.state.read().await;
        Ok(MemorySearchResult {
            hits: search_memory_hits(
                MemoryCorpus {
                    documents: &state.memory_documents,
                    chunks: &state.memory_chunks,
                },
                payload.query.trim(),
                payload.namespace.as_deref(),
                payload.limit.unwrap_or(5),
            ),
        })
    }

    async fn reindex_memory(&self) -> CoreResult<ReindexResult> {
        let mut state = self.state.write().await;
        state.memory_chunks.clear();

        let mut documents = state
            .memory_documents
            .values()
            .cloned()
            .collect::<Vec<MemoryDocumentRecord>>();
        documents.sort_by_key(|document| document.created_at);

        for document in &mut documents {
            let chunks = chunk_memory_document(document);
            document.chunk_count = chunks.len();
            document.updated_at = Utc::now();
            state.memory_chunks.extend(chunks);
            state.memory_documents.insert(document.id, document.clone());
        }

        Ok(ReindexResult {
            documents: state.memory_documents.len(),
            chunks: state.memory_chunks.len(),
        })
    }

    async fn list_mcp_servers(&self) -> CoreResult<Vec<McpServerRecord>> {
        let state = self.state.read().await;
        let mut servers = state.mcp_servers.values().cloned().collect::<Vec<_>>();
        servers.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(servers)
    }

    async fn create_mcp_server(
        &self,
        payload: CreateMcpServerRequest,
    ) -> CoreResult<McpServerRecord> {
        let server = McpServerRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            transport: payload.transport,
            command: payload.command,
            status: ResourceStatus::Active,
            capabilities: vec!["tools.call".into(), "resources.read".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.write().await;
        state.mcp_servers.insert(server.id, server.clone());
        Ok(server)
    }

    async fn get_mcp_server(&self, server_id: Uuid) -> CoreResult<McpServerRecord> {
        let state = self.state.read().await;
        state
            .mcp_servers
            .get(&server_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("mcp server"))
    }

    async fn test_mcp_server(&self, server_id: Uuid) -> CoreResult<TestResult> {
        let state = self.state.read().await;
        let server = state
            .mcp_servers
            .get(&server_id)
            .ok_or_else(|| CoreError::not_found("mcp server"))?;
        Ok(TestResult {
            ok: true,
            message: format!(
                "{} is reachable in this prototype via {} transport.",
                server.name, server.transport
            ),
        })
    }

    async fn get_mcp_capabilities(&self, server_id: Uuid) -> CoreResult<CapabilityEnvelope> {
        let state = self.state.read().await;
        let server = state
            .mcp_servers
            .get(&server_id)
            .ok_or_else(|| CoreError::not_found("mcp server"))?;
        Ok(CapabilityEnvelope {
            capabilities: server.capabilities.clone(),
        })
    }
}
