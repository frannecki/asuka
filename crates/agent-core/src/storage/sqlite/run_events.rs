use diesel::{
    dsl::{exists, select},
    prelude::*,
};
use uuid::Uuid;

use crate::{
    domain::{RunEventEnvelope, RunRecord, RunStatus, RunStreamStatus},
    error::{CoreError, CoreResult},
};

use super::{
    helpers::{load_json_record, load_json_records, serialize_record, sqlite_error},
    store::SqliteStore,
    tables::{agent_run_events, agent_runs, agent_sessions},
};

impl SqliteStore {
    pub(super) async fn get_active_run_db(
        &self,
        session_id: Uuid,
    ) -> CoreResult<Option<RunRecord>> {
        let mut connection = self.open_connection()?;
        let session_exists = select(exists(
            agent_sessions::table.filter(agent_sessions::id.eq(session_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup session", error))?;
        if !session_exists {
            return Err(CoreError::not_found("session"));
        }
        let runs = load_json_records::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::session_id.eq(session_id.to_string()))
                .order(agent_runs::started_at.desc())
                .select(agent_runs::data),
            "run",
        )?;
        Ok(runs
            .into_iter()
            .find(|run| matches!(run.status, RunStatus::Running)))
    }

    pub(super) async fn list_run_events_db(
        &self,
        run_id: Uuid,
        after_sequence: Option<u64>,
    ) -> CoreResult<Vec<RunEventEnvelope>> {
        let mut connection = self.open_connection()?;
        let run_exists = select(exists(
            agent_runs::table.filter(agent_runs::id.eq(run_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup run", error))?;
        if !run_exists {
            return Err(CoreError::not_found("run"));
        }

        match after_sequence {
            Some(after_sequence) => load_json_records(
                &mut connection,
                agent_run_events::table
                    .filter(
                        agent_run_events::run_id
                            .eq(run_id.to_string())
                            .and(agent_run_events::sequence.gt(after_sequence as i64)),
                    )
                    .order(agent_run_events::sequence.asc())
                    .select(agent_run_events::data),
                "run event",
            ),
            None => load_json_records(
                &mut connection,
                agent_run_events::table
                    .filter(agent_run_events::run_id.eq(run_id.to_string()))
                    .order(agent_run_events::sequence.asc())
                    .select(agent_run_events::data),
                "run event",
            ),
        }
    }

    pub(super) async fn append_run_event_db(&self, event: RunEventEnvelope) -> CoreResult<()> {
        let mut connection = self.open_connection()?;
        let run_id = event.run_id.to_string();
        let run_exists = select(exists(
            agent_runs::table.filter(agent_runs::id.eq(run_id.clone())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup run", error))?;
        if !run_exists {
            return Err(CoreError::not_found("run"));
        }

        connection.transaction::<_, CoreError, _>(|transaction| {
            diesel::insert_into(agent_run_events::table)
                .values((
                    agent_run_events::id.eq(Uuid::new_v4().to_string()),
                    agent_run_events::run_id.eq(event.run_id.to_string()),
                    agent_run_events::session_id.eq(event.session_id.to_string()),
                    agent_run_events::sequence.eq(event.sequence as i64),
                    agent_run_events::event_type.eq(event.event_type.clone()),
                    agent_run_events::created_at.eq(event.timestamp.to_rfc3339()),
                    agent_run_events::data.eq(serialize_record(&event, "run event")?),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("insert run event", error))?;
            let mut run = load_json_record::<RunRecord, _>(
                transaction,
                agent_runs::table
                    .filter(agent_runs::id.eq(event.run_id.to_string()))
                    .select(agent_runs::data),
                "run",
            )?;
            run.last_event_sequence = event.sequence;
            run.stream_status = match event.event_type.as_str() {
                "run.completed" => RunStreamStatus::Completed,
                "run.failed" => RunStreamStatus::Failed,
                _ if matches!(run.status, RunStatus::Cancelled) => RunStreamStatus::Cancelled,
                _ => RunStreamStatus::Active,
            };
            if matches!(event.event_type.as_str(), "run.completed" | "run.failed") {
                run.active_stream_message_id = None;
            }
            diesel::update(agent_runs::table.filter(agent_runs::id.eq(run.id.to_string())))
                .set((
                    agent_runs::finished_at.eq(run.finished_at.map(|value| value.to_rfc3339())),
                    agent_runs::data.eq(serialize_record(&run, "run")?),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("update run stream state", error))?;
            Ok(())
        })?;
        Ok(())
    }
}
