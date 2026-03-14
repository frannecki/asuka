use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{CreateSkillRequest, SkillRecord, UpdateSkillRequest},
    error::CoreResult,
};

impl AgentCore {
    pub async fn list_skills(&self) -> CoreResult<Vec<SkillRecord>> {
        self.store.list_skills().await
    }

    pub async fn create_skill(&self, payload: CreateSkillRequest) -> CoreResult<SkillRecord> {
        self.store.create_skill(payload).await
    }

    pub async fn update_skill(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        self.store.update_skill(skill_id, payload).await
    }
}
