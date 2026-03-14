use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{CapabilityEnvelope, CreateMcpServerRequest, McpServerRecord, TestResult},
    error::CoreResult,
};

impl AgentCore {
    pub async fn list_mcp_servers(&self) -> CoreResult<Vec<McpServerRecord>> {
        self.store.list_mcp_servers().await
    }

    pub async fn create_mcp_server(
        &self,
        payload: CreateMcpServerRequest,
    ) -> CoreResult<McpServerRecord> {
        self.store.create_mcp_server(payload).await
    }

    pub async fn get_mcp_server(&self, server_id: Uuid) -> CoreResult<McpServerRecord> {
        self.store.get_mcp_server(server_id).await
    }

    pub async fn test_mcp_server(&self, server_id: Uuid) -> CoreResult<TestResult> {
        self.store.test_mcp_server(server_id).await
    }

    pub async fn get_mcp_capabilities(&self, server_id: Uuid) -> CoreResult<CapabilityEnvelope> {
        self.store.get_mcp_capabilities(server_id).await
    }
}
