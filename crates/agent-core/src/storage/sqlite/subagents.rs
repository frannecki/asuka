use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::{domain::*, error::CoreResult};

use super::{
    helpers::{
        get_json_record_by_id, query_json_records, serialize_record, sqlite_error, update_named_row,
    },
    store::SqliteStore,
};

impl SqliteStore {
    pub(super) async fn list_subagents_db(&self) -> CoreResult<Vec<SubagentRecord>> {
        let connection = self.open_connection()?;
        query_json_records(
            &connection,
            "SELECT data FROM agent_subagents ORDER BY updated_at DESC",
            [],
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

        let connection = self.open_connection()?;
        let data = serialize_record(&subagent, "subagent")?;
        connection
            .execute(
                r#"
                INSERT INTO agent_subagents (id, name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![
                    subagent.id.to_string(),
                    subagent.name,
                    subagent.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("insert subagent", error))?;
        Ok(subagent)
    }

    pub(super) async fn get_subagent_db(&self, subagent_id: Uuid) -> CoreResult<SubagentRecord> {
        let connection = self.open_connection()?;
        get_json_record_by_id(&connection, "agent_subagents", subagent_id, "subagent")
    }

    pub(super) async fn update_subagent_db(
        &self,
        subagent_id: Uuid,
        payload: UpdateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        let connection = self.open_connection()?;
        let mut subagent = get_json_record_by_id::<SubagentRecord>(
            &connection,
            "agent_subagents",
            subagent_id,
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
        update_named_row(
            &connection,
            "agent_subagents",
            "name",
            &subagent.name,
            subagent.id,
            subagent.updated_at.to_rfc3339(),
            &subagent,
            "subagent",
        )?;
        Ok(subagent)
    }
}
