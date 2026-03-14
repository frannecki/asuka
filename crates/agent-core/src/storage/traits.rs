use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    domain::{
        CapabilityEnvelope, CreateMcpServerRequest, CreateMemoryDocumentRequest,
        CreateProviderRequest, CreateSessionRequest, CreateSkillRequest, CreateSubagentRequest,
        McpServerRecord, MemoryDocumentDetail, MemoryDocumentRecord, MemorySearchHit,
        MemorySearchRequest, MemorySearchResult, MessageRecord, PostMessageRequest,
        ProviderAccountRecord, ProviderModelRecord, ReindexResult, RunAccepted, RunRecord,
        SessionDetail, SessionRecord, SkillRecord, SubagentRecord, TestResult,
        UpdateProviderRequest, UpdateSessionRequest, UpdateSkillRequest, UpdateSubagentRequest,
    },
    error::CoreResult,
};

#[derive(Clone)]
pub struct RunContext {
    pub providers: Vec<ProviderAccountRecord>,
    pub recent_messages: Vec<MessageRecord>,
    pub memory_hits: Vec<MemorySearchHit>,
}

#[async_trait]
pub trait AgentStore: Send + Sync {
    async fn list_sessions(&self) -> CoreResult<Vec<SessionRecord>>;
    async fn create_session(&self, payload: CreateSessionRequest) -> CoreResult<SessionRecord>;
    async fn get_session(&self, session_id: Uuid) -> CoreResult<SessionDetail>;
    async fn update_session(
        &self,
        session_id: Uuid,
        payload: UpdateSessionRequest,
    ) -> CoreResult<SessionRecord>;
    async fn delete_session(&self, session_id: Uuid) -> CoreResult<()>;
    async fn list_messages(&self, session_id: Uuid) -> CoreResult<Vec<MessageRecord>>;
    async fn enqueue_user_message(
        &self,
        session_id: Uuid,
        payload: PostMessageRequest,
    ) -> CoreResult<RunAccepted>;
    async fn get_run(&self, run_id: Uuid) -> CoreResult<RunRecord>;
    async fn cancel_run(&self, run_id: Uuid) -> CoreResult<RunRecord>;
    async fn fail_run(&self, run_id: Uuid, message: String) -> CoreResult<RunRecord>;
    async fn run_is_active(&self, run_id: Uuid) -> bool;
    async fn set_run_selection(
        &self,
        run_id: Uuid,
        provider_name: String,
        model_name: String,
    ) -> CoreResult<()>;
    async fn build_run_context(
        &self,
        session_id: Uuid,
        user_content: &str,
    ) -> CoreResult<RunContext>;
    async fn append_assistant_message_and_complete_run(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        response: String,
    ) -> CoreResult<MessageRecord>;
    async fn write_run_memory_note(
        &self,
        user_content: &str,
        response: &str,
    ) -> CoreResult<MemoryDocumentRecord>;

    async fn list_skills(&self) -> CoreResult<Vec<SkillRecord>>;
    async fn create_skill(&self, payload: CreateSkillRequest) -> CoreResult<SkillRecord>;
    async fn update_skill(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord>;

    async fn list_subagents(&self) -> CoreResult<Vec<SubagentRecord>>;
    async fn create_subagent(&self, payload: CreateSubagentRequest) -> CoreResult<SubagentRecord>;
    async fn get_subagent(&self, subagent_id: Uuid) -> CoreResult<SubagentRecord>;
    async fn update_subagent(
        &self,
        subagent_id: Uuid,
        payload: UpdateSubagentRequest,
    ) -> CoreResult<SubagentRecord>;

    async fn list_providers(&self) -> CoreResult<Vec<ProviderAccountRecord>>;
    async fn create_provider(
        &self,
        payload: CreateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord>;
    async fn get_provider(&self, provider_id: Uuid) -> CoreResult<ProviderAccountRecord>;
    async fn update_provider(
        &self,
        provider_id: Uuid,
        payload: UpdateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord>;
    async fn list_provider_models(&self, provider_id: Uuid)
        -> CoreResult<Vec<ProviderModelRecord>>;
    async fn replace_provider_models(
        &self,
        provider_id: Uuid,
        base_url: Option<String>,
        models: Vec<ProviderModelRecord>,
    ) -> CoreResult<ProviderAccountRecord>;

    async fn list_memory_documents(&self) -> CoreResult<Vec<MemoryDocumentRecord>>;
    async fn create_memory_document(
        &self,
        payload: CreateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord>;
    async fn get_memory_document(&self, document_id: Uuid) -> CoreResult<MemoryDocumentDetail>;
    async fn search_memory(&self, payload: MemorySearchRequest) -> CoreResult<MemorySearchResult>;
    async fn reindex_memory(&self) -> CoreResult<ReindexResult>;

    async fn list_mcp_servers(&self) -> CoreResult<Vec<McpServerRecord>>;
    async fn create_mcp_server(
        &self,
        payload: CreateMcpServerRequest,
    ) -> CoreResult<McpServerRecord>;
    async fn get_mcp_server(&self, server_id: Uuid) -> CoreResult<McpServerRecord>;
    async fn test_mcp_server(&self, server_id: Uuid) -> CoreResult<TestResult>;
    async fn get_mcp_capabilities(&self, server_id: Uuid) -> CoreResult<CapabilityEnvelope>;
}
