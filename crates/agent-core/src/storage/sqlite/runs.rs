use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::{
    domain::*,
    error::CoreResult,
    memory::{search_memory_hits, MemoryCorpus},
    storage::RunContext,
};

use super::{
    helpers::{
        ensure_row_exists, get_json_record_by_id, query_json_records, serialize_record,
        sqlite_error, update_run_row,
    },
    store::SqliteStore,
};

impl SqliteStore {
    pub(super) async fn get_run_db(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let connection = self.open_connection()?;
        get_json_record_by_id(&connection, "agent_runs", run_id, "run")
    }

    pub(super) async fn cancel_run_db(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let connection = self.open_connection()?;
        let mut run = get_json_record_by_id::<RunRecord>(&connection, "agent_runs", run_id, "run")?;
        run.status = RunStatus::Cancelled;
        run.finished_at = Some(Utc::now());
        update_run_row(&connection, &run)?;
        Ok(run)
    }

    pub(super) async fn fail_run_db(&self, run_id: Uuid, message: String) -> CoreResult<RunRecord> {
        let connection = self.open_connection()?;
        let mut run = get_json_record_by_id::<RunRecord>(&connection, "agent_runs", run_id, "run")?;
        run.status = RunStatus::Failed;
        run.finished_at = Some(Utc::now());
        run.error = Some(message);
        update_run_row(&connection, &run)?;
        Ok(run)
    }

    pub(super) async fn run_is_active_db(&self, run_id: Uuid) -> bool {
        let connection = match self.open_connection() {
            Ok(connection) => connection,
            Err(_) => return false,
        };
        match get_json_record_by_id::<RunRecord>(&connection, "agent_runs", run_id, "run") {
            Ok(run) => matches!(run.status, RunStatus::Running),
            Err(_) => false,
        }
    }

    pub(super) async fn set_run_selection_db(
        &self,
        run_id: Uuid,
        provider_name: String,
        model_name: String,
    ) -> CoreResult<()> {
        let connection = self.open_connection()?;
        let mut run = get_json_record_by_id::<RunRecord>(&connection, "agent_runs", run_id, "run")?;
        run.selected_provider = Some(provider_name);
        run.selected_model = Some(model_name);
        update_run_row(&connection, &run)
    }

    pub(super) async fn build_run_context_db(
        &self,
        session_id: Uuid,
        user_content: &str,
    ) -> CoreResult<RunContext> {
        let connection = self.open_connection()?;
        ensure_row_exists(&connection, "agent_sessions", session_id, "session")?;

        let mut recent_messages = query_json_records(
            &connection,
            "SELECT data FROM agent_messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 6",
            [session_id.to_string()],
            "message",
        )?;
        recent_messages.reverse();

        let providers = query_json_records(
            &connection,
            "SELECT data FROM agent_providers ORDER BY updated_at DESC",
            [],
            "provider",
        )?;

        let memory_hits = match self.search_memory_semantic(user_content, None, 3).await? {
            Some(hits) => hits,
            None => {
                let state = self.load_memory_state()?;
                search_memory_hits(
                    MemoryCorpus {
                        documents: &state.memory_documents,
                        chunks: &state.memory_chunks,
                    },
                    user_content,
                    None,
                    3,
                )
            }
        };

        Ok(RunContext {
            providers,
            recent_messages,
            memory_hits,
        })
    }

    pub(super) async fn append_assistant_message_and_complete_run_db(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        response: String,
    ) -> CoreResult<MessageRecord> {
        let assistant_message = MessageRecord {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::Assistant,
            content: response,
            created_at: Utc::now(),
            run_id: Some(run_id),
        };

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction().map_err(|error| {
            sqlite_error(
                "begin append assistant message and complete run transaction",
                error,
            )
        })?;

        ensure_row_exists(&transaction, "agent_sessions", session_id, "session")?;
        let mut run =
            get_json_record_by_id::<RunRecord>(&transaction, "agent_runs", run_id, "run")?;
        if !matches!(run.status, RunStatus::Running) {
            return Err(crate::error::CoreError::conflict("run is no longer active"));
        }

        let message_data = serialize_record(&assistant_message, "assistant message")?;
        transaction
            .execute(
                r#"
                INSERT INTO agent_messages (id, session_id, run_id, created_at, data)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    assistant_message.id.to_string(),
                    assistant_message.session_id.to_string(),
                    assistant_message.run_id.map(|value| value.to_string()),
                    assistant_message.created_at.to_rfc3339(),
                    message_data
                ],
            )
            .map_err(|error| sqlite_error("insert assistant message", error))?;

        run.status = RunStatus::Completed;
        run.finished_at = Some(Utc::now());
        run.error = None;
        let run_data = serialize_record(&run, "run")?;
        transaction
            .execute(
                r#"
                UPDATE agent_runs
                SET finished_at = ?2, data = ?3
                WHERE id = ?1
                "#,
                params![
                    run.id.to_string(),
                    run.finished_at.map(|value| value.to_rfc3339()),
                    run_data
                ],
            )
            .map_err(|error| sqlite_error("update run during completion", error))?;

        let mut session = get_json_record_by_id::<SessionRecord>(
            &transaction,
            "agent_sessions",
            session_id,
            "session",
        )?;
        session.updated_at = Utc::now();
        session.summary = "Last run completed through the agent-core runtime.".to_string();
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
            .map_err(|error| sqlite_error("update session during completion", error))?;

        transaction.commit().map_err(|error| {
            sqlite_error(
                "commit append assistant message and complete run transaction",
                error,
            )
        })?;

        Ok(assistant_message)
    }
}
