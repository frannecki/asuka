use std::time::Duration;

use serde_json::json;
use tokio::time::sleep;
use uuid::Uuid;

use crate::{core::AgentCore, error::CoreResult, memory::chunk_text};

impl AgentCore {
    pub(crate) async fn stream_response_deltas(
        &self,
        run_id: Uuid,
        session_id: Uuid,
        response: &str,
    ) {
        for chunk in chunk_text(response, 32) {
            if !self.run_is_active(run_id).await {
                return;
            }

            sleep(Duration::from_millis(35)).await;
            self.publish_event(
                "message.delta",
                run_id,
                session_id,
                json!({ "delta": chunk }),
            );
        }
    }

    pub(crate) async fn finalize_run(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        user_content: &str,
        response: String,
    ) -> CoreResult<()> {
        if !self.run_is_active(run_id).await {
            return Ok(());
        }

        let assistant_message = match self
            .store
            .append_assistant_message_and_complete_run(session_id, run_id, response.clone())
            .await
        {
            Ok(message) => message,
            Err(error) if error.status == 409 => return Ok(()),
            Err(error) => return Err(error),
        };

        if should_write_memory_note(user_content) {
            match self
                .store
                .write_run_memory_note(user_content, &response)
                .await
            {
                Ok(memory_document) => {
                    self.publish_event(
                        "memory.written",
                        run_id,
                        session_id,
                        json!({
                            "documentId": memory_document.id,
                            "title": memory_document.title,
                            "namespace": memory_document.namespace,
                            "chunkCount": memory_document.chunk_count
                        }),
                    );
                }
                Err(error) => {
                    self.publish_event(
                        "run.step.started",
                        run_id,
                        session_id,
                        json!({
                            "stepType": "memory-write-skipped",
                            "message": error.message
                        }),
                    );
                }
            }
        }

        self.publish_event(
            "run.completed",
            run_id,
            session_id,
            json!({
                "status": "completed",
                "messageId": assistant_message.id
            }),
        );

        Ok(())
    }
}

fn should_write_memory_note(user_content: &str) -> bool {
    user_content
        .to_lowercase()
        .split_whitespace()
        .any(|term| matches!(term, "remember" | "save" | "store" | "memorize"))
}
