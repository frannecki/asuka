use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{CreateSubagentRequest, SubagentRecord, UpdateSubagentRequest},
    error::CoreResult,
};

impl AgentCore {
    pub async fn list_subagents(&self) -> CoreResult<Vec<SubagentRecord>> {
        self.store.list_subagents().await
    }

    pub async fn create_subagent(
        &self,
        payload: CreateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        self.store.create_subagent(payload).await
    }

    pub async fn get_subagent(&self, subagent_id: Uuid) -> CoreResult<SubagentRecord> {
        self.store.get_subagent(subagent_id).await
    }

    pub async fn update_subagent(
        &self,
        subagent_id: Uuid,
        payload: UpdateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        self.store.update_subagent(subagent_id, payload).await
    }
}
