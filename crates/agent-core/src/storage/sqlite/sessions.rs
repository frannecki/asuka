use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::{
    domain::*,
    error::{CoreError, CoreResult},
};

use super::{
    helpers::{
        get_json_record_by_id, query_json_records, serialize_record, sqlite_error, update_json_row,
    },
    store::SqliteStore,
};

impl SqliteStore {
    pub(super) async fn list_sessions_db(&self) -> CoreResult<Vec<SessionRecord>> {
        let connection = self.open_connection()?;
        query_json_records(
            &connection,
            "SELECT data FROM agent_sessions ORDER BY updated_at DESC",
            [],
            "session",
        )
    }

    pub(super) async fn create_session_db(
        &self,
        payload: CreateSessionRequest,
    ) -> CoreResult<SessionRecord> {
        let session = SessionRecord {
            id: Uuid::new_v4(),
            title: payload
                .title
                .unwrap_or_else(|| format!("Session {}", Utc::now().format("%H:%M:%S"))),
            status: SessionStatus::Active,
            root_agent_id: "default-root-agent".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_run_at: None,
            summary: "New session".to_string(),
        };

        let connection = self.open_connection()?;
        let data = serialize_record(&session, "session")?;
        connection
            .execute(
                r#"
                INSERT INTO agent_sessions (id, created_at, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![
                    session.id.to_string(),
                    session.created_at.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("insert session", error))?;
        Ok(session)
    }

    pub(super) async fn get_session_db(&self, session_id: Uuid) -> CoreResult<SessionDetail> {
        let connection = self.open_connection()?;
        let session = get_json_record_by_id::<SessionRecord>(
            &connection,
            "agent_sessions",
            session_id,
            "session",
        )?;
        let messages = query_json_records(
            &connection,
            "SELECT data FROM agent_messages WHERE session_id = ?1 ORDER BY created_at ASC",
            [session_id.to_string()],
            "message",
        )?;
        Ok(SessionDetail { session, messages })
    }

    pub(super) async fn update_session_db(
        &self,
        session_id: Uuid,
        payload: UpdateSessionRequest,
    ) -> CoreResult<SessionRecord> {
        let connection = self.open_connection()?;
        let mut session = get_json_record_by_id::<SessionRecord>(
            &connection,
            "agent_sessions",
            session_id,
            "session",
        )?;
        if let Some(title) = payload.title {
            session.title = title;
        }
        if let Some(status) = payload.status {
            session.status = status;
        }
        session.updated_at = Utc::now();
        update_json_row(
            &connection,
            "agent_sessions",
            session.id,
            session.updated_at.to_rfc3339(),
            &session,
            "session",
        )?;
        Ok(session)
    }

    pub(super) async fn delete_session_db(&self, session_id: Uuid) -> CoreResult<()> {
        let connection = self.open_connection()?;
        let deleted = connection
            .execute(
                "DELETE FROM agent_sessions WHERE id = ?1",
                [session_id.to_string()],
            )
            .map_err(|error| sqlite_error("delete session", error))?;
        if deleted == 0 {
            return Err(CoreError::not_found("session"));
        }
        Ok(())
    }

    pub(super) async fn list_messages_db(
        &self,
        session_id: Uuid,
    ) -> CoreResult<Vec<MessageRecord>> {
        let connection = self.open_connection()?;
        super::helpers::ensure_row_exists(&connection, "agent_sessions", session_id, "session")?;
        query_json_records(
            &connection,
            "SELECT data FROM agent_messages WHERE session_id = ?1 ORDER BY created_at ASC",
            [session_id.to_string()],
            "message",
        )
    }

    pub(super) async fn enqueue_user_message_db(
        &self,
        session_id: Uuid,
        payload: PostMessageRequest,
    ) -> CoreResult<RunAccepted> {
        if payload.content.trim().is_empty() {
            return Err(CoreError::bad_request("message content cannot be empty"));
        }

        let run = RunRecord {
            id: Uuid::new_v4(),
            session_id,
            trigger_type: "userMessage".to_string(),
            status: RunStatus::Running,
            selected_provider: None,
            selected_model: None,
            started_at: Utc::now(),
            finished_at: None,
            error: None,
        };
        let user_message = MessageRecord {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::User,
            content: payload.content.trim().to_string(),
            created_at: Utc::now(),
            run_id: Some(run.id),
        };

        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|error| sqlite_error("begin enqueue user message transaction", error))?;

        let mut session = get_json_record_by_id::<SessionRecord>(
            &transaction,
            "agent_sessions",
            session_id,
            "session",
        )?;
        session.updated_at = Utc::now();
        session.last_run_at = Some(Utc::now());

        let session_data = serialize_record(&session, "session")?;
        transaction
            .execute(
                r#"
                UPDATE agent_sessions
                SET updated_at = ?2, data = ?3
                WHERE id = ?1
                "#,
                params![
                    session.id.to_string(),
                    session.updated_at.to_rfc3339(),
                    session_data
                ],
            )
            .map_err(|error| sqlite_error("update session during enqueue", error))?;

        let run_data = serialize_record(&run, "run")?;
        transaction
            .execute(
                r#"
                INSERT INTO agent_runs (id, session_id, started_at, finished_at, data)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    run.id.to_string(),
                    run.session_id.to_string(),
                    run.started_at.to_rfc3339(),
                    Option::<String>::None,
                    run_data
                ],
            )
            .map_err(|error| sqlite_error("insert run", error))?;

        let message_data = serialize_record(&user_message, "message")?;
        transaction
            .execute(
                r#"
                INSERT INTO agent_messages (id, session_id, run_id, created_at, data)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    user_message.id.to_string(),
                    user_message.session_id.to_string(),
                    user_message.run_id.map(|value| value.to_string()),
                    user_message.created_at.to_rfc3339(),
                    message_data
                ],
            )
            .map_err(|error| sqlite_error("insert user message", error))?;

        transaction
            .commit()
            .map_err(|error| sqlite_error("commit enqueue user message transaction", error))?;
        Ok(RunAccepted { run, user_message })
    }
}
