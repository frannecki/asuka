use std::time::Duration;

use serde_json::json;
use tokio::time::sleep;
use uuid::Uuid;

use crate::core::AgentCore;

impl AgentCore {
    pub(crate) async fn maybe_emit_prototype_subagent_activity(
        &self,
        run_id: Uuid,
        session_id: Uuid,
        user_content: &str,
    ) {
        if !(user_content.to_lowercase().contains("subagent") || user_content.len() > 120) {
            return;
        }

        sleep(Duration::from_millis(100)).await;
        self.publish_event(
            "subagent.started",
            run_id,
            session_id,
            json!({
                "subagent": "research-analyst",
                "task": "bounded analysis pass"
            }),
        );
        sleep(Duration::from_millis(180)).await;
        self.publish_event(
            "subagent.completed",
            run_id,
            session_id,
            json!({
                "subagent": "research-analyst",
                "summary": "Subagent produced a scoped summary for the parent run."
            }),
        );
    }
}
