use std::time::Duration;

use serde_json::json;
use tokio::time::sleep;
use uuid::Uuid;

use crate::{core::AgentCore, domain::PlanStepKind, error::CoreResult, memory::summarize_text};

impl AgentCore {
    pub(crate) async fn maybe_emit_prototype_subagent_activity(
        &self,
        run_id: Uuid,
        session_id: Uuid,
        task_id: Uuid,
        user_content: &str,
    ) -> CoreResult<()> {
        if !(user_content.to_lowercase().contains("subagent") || user_content.len() > 120) {
            return Ok(());
        }

        let subagent_name = "research-analyst";
        let step = self
            .store
            .start_run_step(
                run_id,
                None,
                PlanStepKind::Subagent,
                format!("Delegate to {subagent_name}"),
                summarize_text(user_content, 24),
            )
            .await?;

        sleep(Duration::from_millis(100)).await;
        self.publish_event(
            "subagent.started",
            run_id,
            session_id,
            json!({
                "subagent": subagent_name,
                "task": "bounded analysis pass"
            }),
        )
        .await;
        sleep(Duration::from_millis(180)).await;
        let summary = "Subagent produced a scoped summary for the parent run.";
        self.write_subagent_artifact(session_id, task_id, run_id, step.id, subagent_name, summary)
            .await?;
        self.store
            .complete_run_step(step.id, summarize_text(summary, 24))
            .await?;
        self.publish_event(
            "subagent.completed",
            run_id,
            session_id,
            json!({
                "subagent": subagent_name,
                "summary": summary
            }),
        )
        .await;

        Ok(())
    }
}
