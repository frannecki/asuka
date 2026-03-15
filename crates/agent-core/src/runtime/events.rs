use std::sync::atomic::Ordering;

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{core::AgentCore, domain::RunEventEnvelope};

impl AgentCore {
    pub fn stream_ready_event(&self, run_id: Uuid, session_id: Uuid) -> RunEventEnvelope {
        RunEventEnvelope {
            event_type: "run.stream.ready".to_string(),
            run_id,
            session_id,
            timestamp: Utc::now(),
            sequence: self.event_sequence.fetch_add(1, Ordering::Relaxed),
            payload: json!({ "message": "stream connected" }),
        }
    }

    pub(crate) async fn publish_event(
        &self,
        event_type: &str,
        run_id: Uuid,
        session_id: Uuid,
        payload: Value,
    ) {
        let event = RunEventEnvelope {
            event_type: event_type.to_string(),
            run_id,
            session_id,
            timestamp: Utc::now(),
            sequence: self.event_sequence.fetch_add(1, Ordering::Relaxed),
            payload,
        };

        if let Err(error) = self.store.append_run_event(event.clone()).await {
            tracing::warn!(
                run_id = %run_id,
                session_id = %session_id,
                event_type = %event_type,
                "failed to persist run event: {}",
                error.message
            );
        }
        let _ = self.event_tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use crate::test_support::{create_test_core, multi_provider_config_toml};

    #[tokio::test(flavor = "current_thread")]
    async fn run_events_increment_sequence_and_broadcast_in_order() {
        let core = create_test_core(multi_provider_config_toml());
        let mut rx = core.subscribe_events();
        let run_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();

        let ready = core.stream_ready_event(run_id, session_id);
        core.publish_event("run.started", run_id, session_id, json!({ "step": 1 }))
            .await;
        core.publish_event("run.completed", run_id, session_id, json!({ "step": 2 }))
            .await;

        let started = rx.try_recv().expect("receive started event");
        let completed = rx.try_recv().expect("receive completed event");

        assert_eq!(ready.sequence, 1);
        assert_eq!(started.sequence, 2);
        assert_eq!(completed.sequence, 3);
        assert_eq!(started.event_type, "run.started");
        assert_eq!(completed.event_type, "run.completed");
    }
}
