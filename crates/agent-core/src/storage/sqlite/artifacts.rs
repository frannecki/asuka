use diesel::{
    dsl::{exists, select},
    prelude::*,
    upsert::excluded,
};
use uuid::Uuid;

use crate::{
    domain::ArtifactRecord,
    error::{CoreError, CoreResult},
};

use super::{
    helpers::{load_json_records, serialize_record, sqlite_error},
    store::SqliteStore,
    tables::{agent_artifacts, agent_runs, agent_sessions, agent_tasks},
};

impl SqliteStore {
    pub(super) async fn list_session_artifacts_db(
        &self,
        session_id: Uuid,
    ) -> CoreResult<Vec<ArtifactRecord>> {
        let mut connection = self.open_connection()?;
        let session_exists = select(exists(
            agent_sessions::table.filter(agent_sessions::id.eq(session_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup session", error))?;
        if !session_exists {
            return Err(CoreError::not_found("session"));
        }
        load_json_records(
            &mut connection,
            agent_artifacts::table
                .filter(agent_artifacts::session_id.eq(session_id.to_string()))
                .order((
                    agent_artifacts::updated_at.desc(),
                    agent_artifacts::path.asc(),
                ))
                .select(agent_artifacts::data),
            "artifact",
        )
    }

    pub(super) async fn list_task_artifacts_db(
        &self,
        task_id: Uuid,
    ) -> CoreResult<Vec<ArtifactRecord>> {
        let mut connection = self.open_connection()?;
        let task_exists = select(exists(
            agent_tasks::table.filter(agent_tasks::id.eq(task_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup task", error))?;
        if !task_exists {
            return Err(CoreError::not_found("task"));
        }
        load_json_records(
            &mut connection,
            agent_artifacts::table
                .filter(agent_artifacts::task_id.eq(task_id.to_string()))
                .order((
                    agent_artifacts::updated_at.desc(),
                    agent_artifacts::path.asc(),
                ))
                .select(agent_artifacts::data),
            "artifact",
        )
    }

    pub(super) async fn list_run_artifacts_db(
        &self,
        run_id: Uuid,
    ) -> CoreResult<Vec<ArtifactRecord>> {
        let mut connection = self.open_connection()?;
        let run_exists = select(exists(
            agent_runs::table.filter(agent_runs::id.eq(run_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup run", error))?;
        if !run_exists {
            return Err(CoreError::not_found("run"));
        }
        load_json_records(
            &mut connection,
            agent_artifacts::table
                .filter(agent_artifacts::run_id.eq(run_id.to_string()))
                .order((
                    agent_artifacts::updated_at.desc(),
                    agent_artifacts::path.asc(),
                ))
                .select(agent_artifacts::data),
            "artifact",
        )
    }

    pub(super) async fn upsert_artifact_db(
        &self,
        artifact: ArtifactRecord,
    ) -> CoreResult<ArtifactRecord> {
        let mut connection = self.open_connection()?;
        let session_exists = select(exists(
            agent_sessions::table.filter(agent_sessions::id.eq(artifact.session_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup session", error))?;
        if !session_exists {
            return Err(CoreError::not_found("session"));
        }
        let task_exists = select(exists(
            agent_tasks::table.filter(agent_tasks::id.eq(artifact.task_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup task", error))?;
        if !task_exists {
            return Err(CoreError::not_found("task"));
        }
        let run_exists = select(exists(
            agent_runs::table.filter(agent_runs::id.eq(artifact.run_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup run", error))?;
        if !run_exists {
            return Err(CoreError::not_found("run"));
        }

        diesel::insert_into(agent_artifacts::table)
            .values((
                agent_artifacts::id.eq(artifact.id.to_string()),
                agent_artifacts::session_id.eq(artifact.session_id.to_string()),
                agent_artifacts::task_id.eq(artifact.task_id.to_string()),
                agent_artifacts::run_id.eq(artifact.run_id.to_string()),
                agent_artifacts::path.eq(artifact.path.clone()),
                agent_artifacts::created_at.eq(artifact.created_at.to_rfc3339()),
                agent_artifacts::updated_at.eq(artifact.updated_at.to_rfc3339()),
                agent_artifacts::data.eq(serialize_record(&artifact, "artifact")?),
            ))
            .on_conflict(agent_artifacts::id)
            .do_update()
            .set((
                agent_artifacts::session_id.eq(excluded(agent_artifacts::session_id)),
                agent_artifacts::task_id.eq(excluded(agent_artifacts::task_id)),
                agent_artifacts::run_id.eq(excluded(agent_artifacts::run_id)),
                agent_artifacts::path.eq(excluded(agent_artifacts::path)),
                agent_artifacts::updated_at.eq(excluded(agent_artifacts::updated_at)),
                agent_artifacts::data.eq(excluded(agent_artifacts::data)),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("upsert artifact", error))?;
        Ok(artifact)
    }
}
