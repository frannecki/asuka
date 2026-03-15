use uuid::Uuid;

use crate::{core::AgentCore, domain::ArtifactRecord, error::CoreResult};

impl AgentCore {
    pub async fn list_session_artifacts(
        &self,
        session_id: Uuid,
    ) -> CoreResult<Vec<ArtifactRecord>> {
        self.store.list_session_artifacts(session_id).await
    }

    pub async fn list_task_artifacts(&self, task_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        self.store.list_task_artifacts(task_id).await
    }

    pub async fn list_run_artifacts(&self, run_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        self.store.list_run_artifacts(run_id).await
    }
}
