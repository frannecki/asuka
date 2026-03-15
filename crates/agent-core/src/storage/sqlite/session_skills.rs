use chrono::Utc;
use diesel::{
    dsl::{exists, max, select},
    prelude::*,
    upsert::excluded,
};
use uuid::Uuid;

use crate::{
    domain::{
        default_skill_presets, resolve_session_skills, SessionSkillBinding, SessionSkillPolicy,
        SessionSkillPolicyMode, SessionSkillsDetail, SkillPreset, SkillRecord,
        UpdateSessionSkillBindingRequest,
    },
    error::{CoreError, CoreResult},
};

use super::{
    helpers::{
        load_json_records, load_optional_json_record, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::{
        agent_session_skill_bindings, agent_session_skill_policies, agent_sessions, agent_skills,
    },
};

impl SqliteStore {
    pub(super) async fn list_skill_presets_db(&self) -> CoreResult<Vec<SkillPreset>> {
        Ok(default_skill_presets())
    }

    pub(super) async fn get_session_skills_db(
        &self,
        session_id: Uuid,
    ) -> CoreResult<SessionSkillsDetail> {
        let mut connection = self.open_connection()?;
        let exists = select(exists(
            agent_sessions::table.filter(agent_sessions::id.eq(session_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup session", error))?;
        if !exists {
            return Err(CoreError::not_found("session"));
        }
        resolve_session_skills_sqlite(&mut connection, session_id)
    }

    pub(super) async fn replace_session_skills_db(
        &self,
        session_id: Uuid,
        detail: SessionSkillsDetail,
    ) -> CoreResult<SessionSkillsDetail> {
        let mut connection = self.open_connection()?;
        connection.transaction::<_, CoreError, _>(|transaction| {
            let session_exists = select(exists(
                agent_sessions::table.filter(agent_sessions::id.eq(session_id.to_string())),
            ))
            .get_result::<bool>(transaction)
            .map_err(|error| sqlite_error("lookup session", error))?;
            if !session_exists {
                return Err(CoreError::not_found("session"));
            }
            upsert_session_skill_policy(transaction, &detail.policy)?;
            diesel::delete(
                agent_session_skill_bindings::table
                    .filter(agent_session_skill_bindings::session_id.eq(session_id.to_string())),
            )
            .execute(transaction)
            .map_err(|error| sqlite_error("delete session skill bindings", error))?;
            for binding in &detail.bindings {
                let skill_exists = select(exists(
                    agent_skills::table.filter(agent_skills::id.eq(binding.skill_id.to_string())),
                ))
                .get_result::<bool>(transaction)
                .map_err(|error| sqlite_error("lookup skill", error))?;
                if !skill_exists {
                    return Err(CoreError::not_found("skill"));
                }
                upsert_session_skill_binding(transaction, binding)?;
            }

            resolve_session_skills_sqlite(transaction, session_id)
        })
    }

    pub(super) async fn update_session_skill_binding_db(
        &self,
        session_id: Uuid,
        skill_id: Uuid,
        payload: UpdateSessionSkillBindingRequest,
    ) -> CoreResult<SessionSkillsDetail> {
        let mut connection = self.open_connection()?;
        connection.transaction::<_, CoreError, _>(|transaction| {
            let session_exists = select(exists(
                agent_sessions::table.filter(agent_sessions::id.eq(session_id.to_string())),
            ))
            .get_result::<bool>(transaction)
            .map_err(|error| sqlite_error("lookup session", error))?;
            if !session_exists {
                return Err(CoreError::not_found("session"));
            }
            let skill_exists = select(exists(
                agent_skills::table.filter(agent_skills::id.eq(skill_id.to_string())),
            ))
            .get_result::<bool>(transaction)
            .map_err(|error| sqlite_error("lookup skill", error))?;
            if !skill_exists {
                return Err(CoreError::not_found("skill"));
            }

            let order_index = match payload.order_index {
                Some(value) => value,
                None => {
                    let value = agent_session_skill_bindings::table
                        .filter(agent_session_skill_bindings::session_id.eq(session_id.to_string()))
                        .select(max(agent_session_skill_bindings::order_index))
                        .first::<Option<i64>>(transaction)
                        .map_err(|error| sqlite_error("load session skill order", error))?;
                    value.unwrap_or(-1) as i32 + 1
                }
            };
            let binding = SessionSkillBinding {
                session_id,
                skill_id,
                availability: payload.availability,
                order_index,
                notes: payload.notes,
                updated_at: Utc::now(),
            };
            upsert_session_skill_binding(transaction, &binding)?;
            resolve_session_skills_sqlite(transaction, session_id)
        })
    }

    pub(super) async fn apply_session_skill_preset_db(
        &self,
        session_id: Uuid,
        preset_id: String,
    ) -> CoreResult<SessionSkillsDetail> {
        if !default_skill_presets()
            .iter()
            .any(|preset| preset.id == preset_id)
        {
            return Err(CoreError::bad_request("unknown skill preset"));
        }

        let mut connection = self.open_connection()?;
        connection.transaction::<_, CoreError, _>(|transaction| {
            let session_exists = select(exists(
                agent_sessions::table.filter(agent_sessions::id.eq(session_id.to_string())),
            ))
            .get_result::<bool>(transaction)
            .map_err(|error| sqlite_error("lookup session", error))?;
            if !session_exists {
                return Err(CoreError::not_found("session"));
            }
            upsert_session_skill_policy(
                transaction,
                &SessionSkillPolicy {
                    session_id,
                    mode: SessionSkillPolicyMode::Preset,
                    preset_id: Some(preset_id.clone()),
                    updated_at: Utc::now(),
                },
            )?;
            diesel::delete(
                agent_session_skill_bindings::table
                    .filter(agent_session_skill_bindings::session_id.eq(session_id.to_string())),
            )
            .execute(transaction)
            .map_err(|error| sqlite_error("clear session skill bindings", error))?;
            resolve_session_skills_sqlite(transaction, session_id)
        })
    }
}

fn resolve_session_skills_sqlite(
    connection: &mut SqliteConnection,
    session_id: Uuid,
) -> CoreResult<SessionSkillsDetail> {
    let policy = load_session_skill_policy(connection, session_id)?;
    let bindings = load_session_skill_bindings(connection, session_id)?;
    let skills = load_json_records::<SkillRecord, _>(
        connection,
        agent_skills::table
            .order(agent_skills::updated_at.desc())
            .select(agent_skills::data),
        "skill",
    )?;

    Ok(resolve_session_skills(session_id, policy, bindings, skills))
}

fn load_session_skill_policy(
    connection: &mut SqliteConnection,
    session_id: Uuid,
) -> CoreResult<Option<SessionSkillPolicy>> {
    load_optional_json_record(
        connection,
        agent_session_skill_policies::table
            .filter(agent_session_skill_policies::session_id.eq(session_id.to_string()))
            .select(agent_session_skill_policies::data),
        "session skill policy",
    )
}

fn load_session_skill_bindings(
    connection: &mut SqliteConnection,
    session_id: Uuid,
) -> CoreResult<Vec<SessionSkillBinding>> {
    load_json_records(
        connection,
        agent_session_skill_bindings::table
            .filter(agent_session_skill_bindings::session_id.eq(session_id.to_string()))
            .order((
                agent_session_skill_bindings::order_index.asc(),
                agent_session_skill_bindings::updated_at.desc(),
            ))
            .select(agent_session_skill_bindings::data),
        "session skill binding",
    )
}

fn upsert_session_skill_policy(
    connection: &mut SqliteConnection,
    policy: &SessionSkillPolicy,
) -> CoreResult<()> {
    diesel::insert_into(agent_session_skill_policies::table)
        .values((
            agent_session_skill_policies::session_id.eq(policy.session_id.to_string()),
            agent_session_skill_policies::updated_at.eq(policy.updated_at.to_rfc3339()),
            agent_session_skill_policies::data
                .eq(serialize_record(policy, "session skill policy")?),
        ))
        .on_conflict(agent_session_skill_policies::session_id)
        .do_update()
        .set((
            agent_session_skill_policies::updated_at
                .eq(excluded(agent_session_skill_policies::updated_at)),
            agent_session_skill_policies::data.eq(excluded(agent_session_skill_policies::data)),
        ))
        .execute(connection)
        .map_err(|error| sqlite_error("upsert session skill policy", error))?;
    Ok(())
}

fn upsert_session_skill_binding(
    connection: &mut SqliteConnection,
    binding: &SessionSkillBinding,
) -> CoreResult<()> {
    diesel::insert_into(agent_session_skill_bindings::table)
        .values((
            agent_session_skill_bindings::session_id.eq(binding.session_id.to_string()),
            agent_session_skill_bindings::skill_id.eq(binding.skill_id.to_string()),
            agent_session_skill_bindings::updated_at.eq(binding.updated_at.to_rfc3339()),
            agent_session_skill_bindings::order_index.eq(binding.order_index as i64),
            agent_session_skill_bindings::data
                .eq(serialize_record(binding, "session skill binding")?),
        ))
        .on_conflict((
            agent_session_skill_bindings::session_id,
            agent_session_skill_bindings::skill_id,
        ))
        .do_update()
        .set((
            agent_session_skill_bindings::updated_at
                .eq(excluded(agent_session_skill_bindings::updated_at)),
            agent_session_skill_bindings::order_index
                .eq(excluded(agent_session_skill_bindings::order_index)),
            agent_session_skill_bindings::data.eq(excluded(agent_session_skill_bindings::data)),
        ))
        .execute(connection)
        .map_err(|error| sqlite_error("upsert session skill binding", error))?;
    Ok(())
}
