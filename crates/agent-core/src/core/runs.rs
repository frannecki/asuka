use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{ActiveRunEnvelope, RunEventHistory, RunRecord, RunStepRecord, ToolInvocationRecord},
    error::CoreResult,
};

impl AgentCore {
    pub async fn get_active_run(&self, session_id: Uuid) -> CoreResult<ActiveRunEnvelope> {
        Ok(ActiveRunEnvelope {
            run: self.store.get_active_run(session_id).await?,
        })
    }

    pub async fn get_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        self.store.get_run(run_id).await
    }

    pub async fn list_run_events(
        &self,
        run_id: Uuid,
        after_sequence: Option<u64>,
    ) -> CoreResult<RunEventHistory> {
        let events = self.store.list_run_events(run_id, after_sequence).await?;
        let last_sequence = events.last().map(|event| event.sequence).unwrap_or(0);
        Ok(RunEventHistory {
            run_id,
            after_sequence,
            events,
            last_sequence,
        })
    }

    pub async fn list_run_steps(&self, run_id: Uuid) -> CoreResult<Vec<RunStepRecord>> {
        self.store.list_run_steps(run_id).await
    }

    pub async fn list_tool_invocations(
        &self,
        run_id: Uuid,
    ) -> CoreResult<Vec<ToolInvocationRecord>> {
        self.store.list_tool_invocations(run_id).await
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
        )
        .await;

        Ok(snapshot)
    }
}
