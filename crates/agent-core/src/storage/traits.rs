use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    domain::{
        ArtifactRecord, CapabilityEnvelope, CreateMcpServerRequest, CreateMemoryDocumentRequest,
        CreateProviderRequest, CreateSessionRequest, CreateSkillRequest, CreateSubagentRequest,
        McpServerRecord, MemoryDocumentDetail, MemoryDocumentRecord, MemorySearchHit,
        MemorySearchRequest, MemorySearchResult, MessageRecord, PlanDetail, PlanStepKind,
        PostMessageRequest, ProviderAccountRecord, ProviderModelRecord, ReindexResult, RunAccepted,
        RunEventEnvelope, RunRecord, RunStepRecord, SessionDetail, SessionRecord,
        SessionSkillsDetail, SkillPreset, SkillRecord, SubagentRecord, TaskRecord, TestResult,
        ToolInvocationRecord, UpdateMemoryDocumentRequest, UpdateProviderRequest,
        UpdateSessionRequest, UpdateSessionSkillBindingRequest, UpdateSkillRequest,
        UpdateSubagentRequest,
    },
    error::CoreResult,
};

#[derive(Clone)]
pub struct RunContext {
    pub providers: Vec<ProviderAccountRecord>,
    pub recent_messages: Vec<MessageRecord>,
    pub memory_hits: Vec<MemorySearchHit>,
    pub effective_skill_names: Vec<String>,
    pub pinned_skill_names: Vec<String>,
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
    async fn get_active_run(&self, session_id: Uuid) -> CoreResult<Option<RunRecord>>;
    async fn get_run(&self, run_id: Uuid) -> CoreResult<RunRecord>;
    async fn cancel_run(&self, run_id: Uuid) -> CoreResult<RunRecord>;
    async fn fail_run(&self, run_id: Uuid, message: String) -> CoreResult<RunRecord>;
    async fn run_is_active(&self, run_id: Uuid) -> bool;
    async fn list_run_events(
        &self,
        run_id: Uuid,
        after_sequence: Option<u64>,
    ) -> CoreResult<Vec<RunEventEnvelope>>;
    async fn append_run_event(&self, event: RunEventEnvelope) -> CoreResult<()>;
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
    async fn list_tasks(&self, session_id: Option<Uuid>) -> CoreResult<Vec<TaskRecord>>;
    async fn get_task(&self, task_id: Uuid) -> CoreResult<TaskRecord>;
    async fn get_task_plan(&self, task_id: Uuid) -> CoreResult<PlanDetail>;
    async fn list_task_runs(&self, task_id: Uuid) -> CoreResult<Vec<RunRecord>>;
    async fn list_session_artifacts(&self, session_id: Uuid) -> CoreResult<Vec<ArtifactRecord>>;
    async fn list_task_artifacts(&self, task_id: Uuid) -> CoreResult<Vec<ArtifactRecord>>;
    async fn list_run_artifacts(&self, run_id: Uuid) -> CoreResult<Vec<ArtifactRecord>>;
    async fn upsert_artifact(&self, artifact: ArtifactRecord) -> CoreResult<ArtifactRecord>;
    async fn list_run_steps(&self, run_id: Uuid) -> CoreResult<Vec<RunStepRecord>>;
    async fn start_run_step(
        &self,
        run_id: Uuid,
        plan_step_id: Option<Uuid>,
        kind: PlanStepKind,
        title: String,
        input_summary: String,
    ) -> CoreResult<RunStepRecord>;
    async fn complete_run_step(
        &self,
        run_step_id: Uuid,
        output_summary: String,
    ) -> CoreResult<RunStepRecord>;
    async fn fail_run_step(&self, run_step_id: Uuid, error: String) -> CoreResult<RunStepRecord>;
    async fn list_tool_invocations(&self, run_id: Uuid) -> CoreResult<Vec<ToolInvocationRecord>>;
    async fn record_tool_invocation(
        &self,
        run_step_id: Uuid,
        tool_name: String,
        tool_source: String,
        arguments_json: Value,
        result_json: Value,
        ok: bool,
        error: Option<String>,
    ) -> CoreResult<ToolInvocationRecord>;
    async fn write_run_memory_note(
        &self,
        session_id: Uuid,
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
    async fn list_skill_presets(&self) -> CoreResult<Vec<SkillPreset>>;
    async fn get_session_skills(&self, session_id: Uuid) -> CoreResult<SessionSkillsDetail>;
    async fn replace_session_skills(
        &self,
        session_id: Uuid,
        detail: SessionSkillsDetail,
    ) -> CoreResult<SessionSkillsDetail>;
    async fn update_session_skill_binding(
        &self,
        session_id: Uuid,
        skill_id: Uuid,
        payload: UpdateSessionSkillBindingRequest,
    ) -> CoreResult<SessionSkillsDetail>;
    async fn apply_session_skill_preset(
        &self,
        session_id: Uuid,
        preset_id: String,
    ) -> CoreResult<SessionSkillsDetail>;

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
    async fn update_memory_document(
        &self,
        document_id: Uuid,
        payload: UpdateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord>;
    async fn delete_memory_document(&self, document_id: Uuid) -> CoreResult<()>;
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
