use serde_json::json;
use uuid::Uuid;

use crate::{core::AgentCore, error::CoreError};

impl AgentCore {
    pub(crate) async fn mark_run_failed(&self, run_id: Uuid, session_id: Uuid, error: CoreError) {
        let message = error.message;
        let _ = self.store.fail_run(run_id, message.clone()).await;
        self.publish_event(
            "run.failed",
            run_id,
            session_id,
            json!({
                "status": "failed",
                "message": message
            }),
        )
        .await;
    }

    pub(crate) async fn run_is_active(&self, run_id: Uuid) -> bool {
        self.store.run_is_active(run_id).await
    }
}
