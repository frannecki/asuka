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
    pub(super) async fn list_skills_db(&self) -> CoreResult<Vec<SkillRecord>> {
        let connection = self.open_connection()?;
        query_json_records(
            &connection,
            "SELECT data FROM agent_skills ORDER BY updated_at DESC",
            [],
            "skill",
        )
    }

    pub(super) async fn create_skill_db(
        &self,
        payload: CreateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        let skill = SkillRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            description: payload.description,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let connection = self.open_connection()?;
        let data = serialize_record(&skill, "skill")?;
        connection
            .execute(
                r#"
                INSERT INTO agent_skills (id, name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![
                    skill.id.to_string(),
                    skill.name,
                    skill.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("insert skill", error))?;
        Ok(skill)
    }

    pub(super) async fn update_skill_db(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        let connection = self.open_connection()?;
        let mut skill =
            get_json_record_by_id::<SkillRecord>(&connection, "agent_skills", skill_id, "skill")?;
        if let Some(description) = payload.description {
            skill.description = description;
        }
        if let Some(status) = payload.status {
            skill.status = status;
        }
        skill.updated_at = Utc::now();
        update_named_row(
            &connection,
            "agent_skills",
            "name",
            &skill.name,
            skill.id,
            skill.updated_at.to_rfc3339(),
            &skill,
            "skill",
        )?;
        Ok(skill)
    }
}
