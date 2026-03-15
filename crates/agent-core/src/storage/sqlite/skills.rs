use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::{domain::*, error::CoreResult};

use super::{
    helpers::{
        expect_changed, load_json_record, load_json_records, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::agent_skills,
};

impl SqliteStore {
    pub(super) async fn list_skills_db(&self) -> CoreResult<Vec<SkillRecord>> {
        let mut connection = self.open_connection()?;
        load_json_records(
            &mut connection,
            agent_skills::table
                .order(agent_skills::updated_at.desc())
                .select(agent_skills::data),
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

        let mut connection = self.open_connection()?;
        diesel::insert_into(agent_skills::table)
            .values((
                agent_skills::id.eq(skill.id.to_string()),
                agent_skills::name.eq(skill.name.clone()),
                agent_skills::updated_at.eq(skill.updated_at.to_rfc3339()),
                agent_skills::data.eq(serialize_record(&skill, "skill")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert skill", error))?;
        Ok(skill)
    }

    pub(super) async fn update_skill_db(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        let mut connection = self.open_connection()?;
        let mut skill = load_json_record::<SkillRecord, _>(
            &mut connection,
            agent_skills::table
                .filter(agent_skills::id.eq(skill_id.to_string()))
                .select(agent_skills::data),
            "skill",
        )?;
        if let Some(description) = payload.description {
            skill.description = description;
        }
        if let Some(status) = payload.status {
            skill.status = status;
        }
        skill.updated_at = Utc::now();

        let updated =
            diesel::update(agent_skills::table.filter(agent_skills::id.eq(skill.id.to_string())))
                .set((
                    agent_skills::name.eq(skill.name.clone()),
                    agent_skills::updated_at.eq(skill.updated_at.to_rfc3339()),
                    agent_skills::data.eq(serialize_record(&skill, "skill")?),
                ))
                .execute(&mut connection)
                .map_err(|error| sqlite_error("update skill", error))?;
        expect_changed(updated, "skill")?;
        Ok(skill)
    }
}
