use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{
        CreateSessionRequest, MessageRecord, PostMessageRequest, RunAccepted, SessionDetail,
        SessionRecord, UpdateSessionRequest,
    },
    error::CoreResult,
};

impl AgentCore {
    pub async fn list_sessions(&self) -> CoreResult<Vec<SessionRecord>> {
        self.store.list_sessions().await
    }

    pub async fn create_session(&self, payload: CreateSessionRequest) -> CoreResult<SessionRecord> {
        self.store.create_session(payload).await
    }

    pub async fn get_session(&self, session_id: Uuid) -> CoreResult<SessionDetail> {
        self.store.get_session(session_id).await
    }

    pub async fn update_session(
        &self,
        session_id: Uuid,
        payload: UpdateSessionRequest,
    ) -> CoreResult<SessionRecord> {
        self.store.update_session(session_id, payload).await
    }

    pub async fn delete_session(&self, session_id: Uuid) -> CoreResult<()> {
        self.store.delete_session(session_id).await
    }

    pub async fn list_messages(&self, session_id: Uuid) -> CoreResult<Vec<MessageRecord>> {
        self.store.list_messages(session_id).await
    }

    pub async fn post_message(
        &self,
        session_id: Uuid,
        payload: PostMessageRequest,
    ) -> CoreResult<RunAccepted> {
        let accepted = self.store.enqueue_user_message(session_id, payload).await?;

        let background_core = self.clone();
        let content = accepted.user_message.content.clone();
        let run_id = accepted.run.id;
        tokio::spawn(async move {
            background_core
                .execute_run(session_id, run_id, content)
                .await;
        });

        Ok(accepted)
    }
}
