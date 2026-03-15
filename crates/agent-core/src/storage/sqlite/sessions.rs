use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::{
    domain::*,
    error::{CoreError, CoreResult},
};

use super::{
    helpers::{
        expect_changed, load_json_record, load_json_records, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::{
        agent_messages, agent_runs, agent_session_skill_policies, agent_sessions, agent_tasks,
    },
    tasks::build_default_task_bundle,
};

impl SqliteStore {
    pub(super) async fn list_sessions_db(&self) -> CoreResult<Vec<SessionRecord>> {
        let mut connection = self.open_connection()?;
        load_json_records(
            &mut connection,
            agent_sessions::table
                .order(agent_sessions::updated_at.desc())
                .select(agent_sessions::data),
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

        let mut connection = self.open_connection()?;
        diesel::insert_into(agent_sessions::table)
            .values((
                agent_sessions::id.eq(session.id.to_string()),
                agent_sessions::created_at.eq(session.created_at.to_rfc3339()),
                agent_sessions::updated_at.eq(session.updated_at.to_rfc3339()),
                agent_sessions::data.eq(serialize_record(&session, "session")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert session", error))?;
        let policy = SessionSkillPolicy::default_for(session.id);
        diesel::insert_into(agent_session_skill_policies::table)
            .values((
                agent_session_skill_policies::session_id.eq(session.id.to_string()),
                agent_session_skill_policies::updated_at.eq(policy.updated_at.to_rfc3339()),
                agent_session_skill_policies::data
                    .eq(serialize_record(&policy, "session skill policy")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert default session skill policy", error))?;
        Ok(session)
    }

    pub(super) async fn get_session_db(&self, session_id: Uuid) -> CoreResult<SessionDetail> {
        let (session, messages, session_runs, session_tasks) = {
            let mut connection = self.open_connection()?;
            let session = load_json_record::<SessionRecord, _>(
                &mut connection,
                agent_sessions::table
                    .filter(agent_sessions::id.eq(session_id.to_string()))
                    .select(agent_sessions::data),
                "session",
            )?;
            let messages = load_json_records(
                &mut connection,
                agent_messages::table
                    .filter(agent_messages::session_id.eq(session_id.to_string()))
                    .order(agent_messages::created_at.asc())
                    .select(agent_messages::data),
                "message",
            )?;
            let session_runs: Vec<RunRecord> = load_json_records(
                &mut connection,
                agent_runs::table
                    .filter(agent_runs::session_id.eq(session_id.to_string()))
                    .order(agent_runs::started_at.desc())
                    .select(agent_runs::data),
                "run",
            )?;
            let session_tasks: Vec<TaskRecord> = load_json_records(
                &mut connection,
                agent_tasks::table
                    .filter(agent_tasks::session_id.eq(session_id.to_string()))
                    .order(agent_tasks::updated_at.desc())
                    .select(agent_tasks::data),
                "task",
            )?;
            (session, messages, session_runs, session_tasks)
        };
        let skills = self.get_session_skills_db(session_id).await?;
        let active_run_summary = session_runs
            .iter()
            .find(|run| matches!(run.status, RunStatus::Running))
            .cloned();
        let latest_run_summary = session_runs.first().cloned();
        let active_task_summary = select_active_task(&session_tasks);
        let latest_stream_checkpoint_summary = if let Some(run) = latest_run_summary.as_ref() {
            let events = self.list_run_events_db(run.id, None).await?;
            build_stream_checkpoint_summary(run, &events)
        } else {
            None
        };
        Ok(SessionDetail {
            session,
            messages,
            skill_summary: summarize_session_skills(&skills),
            active_run_summary,
            latest_run_summary,
            active_task_summary,
            latest_stream_checkpoint_summary,
        })
    }

    pub(super) async fn update_session_db(
        &self,
        session_id: Uuid,
        payload: UpdateSessionRequest,
    ) -> CoreResult<SessionRecord> {
        let mut connection = self.open_connection()?;
        let mut session = load_json_record::<SessionRecord, _>(
            &mut connection,
            agent_sessions::table
                .filter(agent_sessions::id.eq(session_id.to_string()))
                .select(agent_sessions::data),
            "session",
        )?;
        if let Some(title) = payload.title {
            session.title = title;
        }
        if let Some(status) = payload.status {
            session.status = status;
        }
        session.updated_at = Utc::now();
        let updated = diesel::update(
            agent_sessions::table.filter(agent_sessions::id.eq(session.id.to_string())),
        )
        .set((
            agent_sessions::updated_at.eq(session.updated_at.to_rfc3339()),
            agent_sessions::data.eq(serialize_record(&session, "session")?),
        ))
        .execute(&mut connection)
        .map_err(|error| sqlite_error("update session", error))?;
        expect_changed(updated, "session")?;
        Ok(session)
    }

    pub(super) async fn delete_session_db(&self, session_id: Uuid) -> CoreResult<()> {
        let mut connection = self.open_connection()?;
        let deleted = diesel::delete(
            agent_sessions::table.filter(agent_sessions::id.eq(session_id.to_string())),
        )
        .execute(&mut connection)
        .map_err(|error| sqlite_error("delete session", error))?;
        expect_changed(deleted, "session")
    }

    pub(super) async fn list_messages_db(
        &self,
        session_id: Uuid,
    ) -> CoreResult<Vec<MessageRecord>> {
        let mut connection = self.open_connection()?;
        let _session = load_json_record::<SessionRecord, _>(
            &mut connection,
            agent_sessions::table
                .filter(agent_sessions::id.eq(session_id.to_string()))
                .select(agent_sessions::data),
            "session",
        )?;
        load_json_records(
            &mut connection,
            agent_messages::table
                .filter(agent_messages::session_id.eq(session_id.to_string()))
                .order(agent_messages::created_at.asc())
                .select(agent_messages::data),
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
        let skills = self.get_session_skills_db(session_id).await?;

        let run = RunRecord {
            id: Uuid::new_v4(),
            session_id,
            task_id: Uuid::nil(),
            trigger_type: "userMessage".to_string(),
            status: RunStatus::Running,
            selected_provider: None,
            selected_model: None,
            started_at: Utc::now(),
            finished_at: None,
            error: None,
            effective_skill_names: skills
                .effective_skills
                .iter()
                .map(|entry| entry.skill.name.clone())
                .collect(),
            pinned_skill_names: skills
                .effective_skills
                .iter()
                .filter(|entry| entry.is_pinned)
                .map(|entry| entry.skill.name.clone())
                .collect(),
            last_event_sequence: 0,
            stream_status: RunStreamStatus::Active,
            active_stream_message_id: Some(Uuid::new_v4()),
        };
        let user_message = MessageRecord {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::User,
            content: payload.content.trim().to_string(),
            created_at: Utc::now(),
            run_id: Some(run.id),
        };
        let (task, plan, steps) = build_default_task_bundle(session_id, &user_message);
        let mut run = run;
        run.task_id = task.id;

        let mut connection = self.open_connection()?;
        connection.transaction::<_, CoreError, _>(|transaction| {
            let mut session = load_json_record::<SessionRecord, _>(
                transaction,
                agent_sessions::table
                    .filter(agent_sessions::id.eq(session_id.to_string()))
                    .select(agent_sessions::data),
                "session",
            )?;
            session.updated_at = Utc::now();
            session.last_run_at = Some(Utc::now());

            diesel::update(
                agent_sessions::table.filter(agent_sessions::id.eq(session.id.to_string())),
            )
            .set((
                agent_sessions::updated_at.eq(session.updated_at.to_rfc3339()),
                agent_sessions::data.eq(serialize_record(&session, "session")?),
            ))
            .execute(transaction)
            .map_err(|error| sqlite_error("update session during enqueue", error))?;

            SqliteStore::insert_task_bundle_sqlite(transaction, &task, &plan, &steps)?;

            diesel::insert_into(agent_runs::table)
                .values((
                    agent_runs::id.eq(run.id.to_string()),
                    agent_runs::session_id.eq(run.session_id.to_string()),
                    agent_runs::task_id.eq(run.task_id.to_string()),
                    agent_runs::started_at.eq(run.started_at.to_rfc3339()),
                    agent_runs::finished_at.eq(Option::<String>::None),
                    agent_runs::data.eq(serialize_record(&run, "run")?),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("insert run", error))?;

            diesel::insert_into(agent_messages::table)
                .values((
                    agent_messages::id.eq(user_message.id.to_string()),
                    agent_messages::session_id.eq(user_message.session_id.to_string()),
                    agent_messages::run_id.eq(user_message.run_id.map(|value| value.to_string())),
                    agent_messages::created_at.eq(user_message.created_at.to_rfc3339()),
                    agent_messages::data.eq(serialize_record(&user_message, "message")?),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("insert user message", error))?;

            Ok(())
        })?;
        Ok(RunAccepted { run, user_message })
    }
}

fn select_active_task(tasks: &[TaskRecord]) -> Option<TaskRecord> {
    tasks
        .iter()
        .find(|task| {
            matches!(
                task.status,
                TaskStatus::Queued
                    | TaskStatus::Planning
                    | TaskStatus::Running
                    | TaskStatus::WaitingForApproval
                    | TaskStatus::Suspended
            )
        })
        .cloned()
        .or_else(|| tasks.first().cloned())
}

fn build_stream_checkpoint_summary(
    run: &RunRecord,
    events: &[RunEventEnvelope],
) -> Option<StreamCheckpointSummary> {
    let last_event = events.last()?;
    let draft_reply_text = events
        .iter()
        .filter(|event| event.event_type == "message.delta")
        .filter_map(|event| event.payload.get("delta").and_then(|value| value.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    Some(StreamCheckpointSummary {
        run_id: run.id,
        last_sequence: last_event.sequence,
        draft_reply_text,
        updated_at: last_event.timestamp,
        active_stream_message_id: run.active_stream_message_id,
    })
}
