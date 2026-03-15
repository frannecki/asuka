use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::{domain::*, error::CoreResult};

use super::{
    helpers::{
        expect_changed, load_json_record, load_json_records, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::agent_subagents,
};

impl SqliteStore {
    pub(super) async fn list_subagents_db(&self) -> CoreResult<Vec<SubagentRecord>> {
        let mut connection = self.open_connection()?;
        load_json_records(
            &mut connection,
            agent_subagents::table
                .order(agent_subagents::updated_at.desc())
                .select(agent_subagents::data),
            "subagent",
        )
    }

    pub(super) async fn create_subagent_db(
        &self,
        payload: CreateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        let subagent = SubagentRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            description: payload.description,
            scope: payload.scope,
            max_steps: payload.max_steps,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut connection = self.open_connection()?;
        diesel::insert_into(agent_subagents::table)
            .values((
                agent_subagents::id.eq(subagent.id.to_string()),
                agent_subagents::name.eq(subagent.name.clone()),
                agent_subagents::updated_at.eq(subagent.updated_at.to_rfc3339()),
                agent_subagents::data.eq(serialize_record(&subagent, "subagent")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert subagent", error))?;
        Ok(subagent)
    }

    pub(super) async fn get_subagent_db(&self, subagent_id: Uuid) -> CoreResult<SubagentRecord> {
        let mut connection = self.open_connection()?;
        load_json_record(
            &mut connection,
            agent_subagents::table
                .filter(agent_subagents::id.eq(subagent_id.to_string()))
                .select(agent_subagents::data),
            "subagent",
        )
    }

    pub(super) async fn update_subagent_db(
        &self,
        subagent_id: Uuid,
        payload: UpdateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        let mut connection = self.open_connection()?;
        let mut subagent = load_json_record::<SubagentRecord, _>(
            &mut connection,
            agent_subagents::table
                .filter(agent_subagents::id.eq(subagent_id.to_string()))
                .select(agent_subagents::data),
            "subagent",
        )?;
        if let Some(description) = payload.description {
            subagent.description = description;
        }
        if let Some(scope) = payload.scope {
            subagent.scope = scope;
        }
        if let Some(max_steps) = payload.max_steps {
            subagent.max_steps = max_steps;
        }
        if let Some(status) = payload.status {
            subagent.status = status;
        }
        subagent.updated_at = Utc::now();

        let updated = diesel::update(
            agent_subagents::table.filter(agent_subagents::id.eq(subagent.id.to_string())),
        )
        .set((
            agent_subagents::name.eq(subagent.name.clone()),
            agent_subagents::updated_at.eq(subagent.updated_at.to_rfc3339()),
            agent_subagents::data.eq(serialize_record(&subagent, "subagent")?),
        ))
        .execute(&mut connection)
        .map_err(|error| sqlite_error("update subagent", error))?;
        expect_changed(updated, "subagent")?;
        Ok(subagent)
    }
}
