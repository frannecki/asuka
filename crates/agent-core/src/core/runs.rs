use uuid::Uuid;

use crate::{core::AgentCore, domain::RunRecord, error::CoreResult};

impl AgentCore {
    pub async fn get_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        self.store.get_run(run_id).await
    }

    pub async fn cancel_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let snapshot = self.store.cancel_run(run_id).await?;

        self.publish_event(
            "run.failed",
            snapshot.id,
            snapshot.session_id,
            serde_json::json!({
                "status": "cancelled",
                "message": "Run was cancelled by the client."
            }),
        );

        Ok(snapshot)
    }
}
