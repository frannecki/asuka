use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    config::ModelsConfig,
    domain::*,
    memory::{chunk_memory_document, summarize_text},
};

#[derive(Default)]
pub(crate) struct StoreState {
    pub sessions: HashMap<Uuid, SessionRecord>,
    pub messages: HashMap<Uuid, Vec<MessageRecord>>,
    pub runs: HashMap<Uuid, RunRecord>,
    pub skills: HashMap<Uuid, SkillRecord>,
    pub subagents: HashMap<Uuid, SubagentRecord>,
    pub providers: HashMap<Uuid, ProviderAccountRecord>,
    pub memory_documents: HashMap<Uuid, MemoryDocumentRecord>,
    pub memory_chunks: Vec<MemoryChunkRecord>,
    pub mcp_servers: HashMap<Uuid, McpServerRecord>,
}

impl StoreState {
    pub(crate) fn seeded(config: &ModelsConfig) -> Self {
        let mut state = Self::default();

        for configured_provider in &config.providers {
            let provider = ProviderAccountRecord {
                id: Uuid::new_v4(),
                provider_type: configured_provider.provider_type.clone(),
                display_name: configured_provider.display_name.clone(),
                base_url: configured_provider.base_url.clone(),
                status: ResourceStatus::Active,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                models: configured_provider.to_models(),
            };

            state.providers.insert(provider.id, provider);
        }

        let research_skill = SkillRecord {
            id: Uuid::new_v4(),
            name: "research-skill".to_string(),
            description: "Structured source-backed research workflow.".to_string(),
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        state.skills.insert(research_skill.id, research_skill);

        let planner_skill = SkillRecord {
            id: Uuid::new_v4(),
            name: "planning-skill".to_string(),
            description: "Breaks large requests into inspectable execution steps.".to_string(),
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        state.skills.insert(planner_skill.id, planner_skill);

        let subagent = SubagentRecord {
            id: Uuid::new_v4(),
            name: "research-analyst".to_string(),
            description: "Bounded specialist for investigation and synthesis.".to_string(),
            scope: "web research, notes, summarization".to_string(),
            max_steps: 8,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        state.subagents.insert(subagent.id, subagent);

        let seeded_memory_documents = vec![
            seed_memory_document(
                "platform-overview",
                "global",
                "seed",
                "The Asuka platform aims to support a Rust backend service, a Next.js chat UI, skill registration, subagent delegation, MCP integration, context compression, and long-term memory through RAG.",
            ),
            seed_memory_document(
                "provider-policy",
                "global",
                "seed",
                "Mainstream providers should be first-class citizens. The launch set includes Moonshot, OpenAI, Anthropic, Google Gemini, Azure OpenAI, and OpenRouter with capability-aware routing.",
            ),
        ];

        for mut document in seeded_memory_documents {
            let chunks = chunk_memory_document(&document);
            document.chunk_count = chunks.len();
            state.memory_chunks.extend(chunks);
            state.memory_documents.insert(document.id, document);
        }

        let mcp_server = McpServerRecord {
            id: Uuid::new_v4(),
            name: "filesystem".to_string(),
            transport: "stdio".to_string(),
            command: "npx @modelcontextprotocol/server-filesystem".to_string(),
            status: ResourceStatus::Active,
            capabilities: vec![
                "resources.list".to_string(),
                "resources.read".to_string(),
                "tools.call".to_string(),
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        state.mcp_servers.insert(mcp_server.id, mcp_server);

        let session_id = Uuid::new_v4();
        let welcome_message = MessageRecord {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::Assistant,
            content: "The backend is now split into an independent agent-core crate and a thin API layer. If a configured provider such as Moonshot or OpenRouter is available, agent-core will try the configured model before falling back to a local response.".to_string(),
            created_at: Utc::now(),
            run_id: None,
        };

        let session = SessionRecord {
            id: session_id,
            title: "Implementation Sandbox".to_string(),
            status: SessionStatus::Active,
            root_agent_id: "default-root-agent".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_run_at: None,
            summary: "Starter session seeded by agent-core.".to_string(),
        };

        state.sessions.insert(session.id, session);
        state.messages.insert(session_id, vec![welcome_message]);
        state
    }
}

fn seed_memory_document(
    title: &str,
    namespace: &str,
    source: &str,
    content: &str,
) -> MemoryDocumentRecord {
    MemoryDocumentRecord {
        id: Uuid::new_v4(),
        title: title.to_string(),
        namespace: namespace.to_string(),
        source: source.to_string(),
        content: content.to_string(),
        summary: summarize_text(content, 18),
        chunk_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
