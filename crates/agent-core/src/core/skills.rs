use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{
        resolve_session_skills, ApplySkillPresetRequest, CreateSkillRequest,
        ReplaceSessionSkillsRequest, SessionSkillBinding, SessionSkillPolicy, SessionSkillsDetail,
        SkillPreset, SkillRecord, UpdateSessionSkillBindingRequest, UpdateSkillRequest,
    },
    error::CoreResult,
};

impl AgentCore {
    pub async fn list_skills(&self) -> CoreResult<Vec<SkillRecord>> {
        self.store.list_skills().await
    }

    pub async fn create_skill(&self, payload: CreateSkillRequest) -> CoreResult<SkillRecord> {
        self.store.create_skill(payload).await
    }

    pub async fn update_skill(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        self.store.update_skill(skill_id, payload).await
    }

    pub async fn list_skill_presets(&self) -> CoreResult<Vec<SkillPreset>> {
        self.store.list_skill_presets().await
    }

    pub async fn get_session_skills(&self, session_id: Uuid) -> CoreResult<SessionSkillsDetail> {
        self.store.get_session_skills(session_id).await
    }

    pub async fn replace_session_skills(
        &self,
        session_id: Uuid,
        payload: ReplaceSessionSkillsRequest,
    ) -> CoreResult<SessionSkillsDetail> {
        let skills = self.store.list_skills().await?;
        let detail = resolve_session_skills(
            session_id,
            Some(SessionSkillPolicy {
                session_id,
                mode: payload.mode,
                preset_id: payload.preset_id,
                updated_at: chrono::Utc::now(),
            }),
            payload
                .bindings
                .into_iter()
                .enumerate()
                .map(|(index, binding)| SessionSkillBinding {
                    session_id,
                    skill_id: binding.skill_id,
                    availability: binding.availability,
                    order_index: binding.order_index.unwrap_or(index as i32),
                    notes: binding.notes,
                    updated_at: chrono::Utc::now(),
                })
                .collect(),
            skills,
        );

        self.store.replace_session_skills(session_id, detail).await
    }

    pub async fn update_session_skill_binding(
        &self,
        session_id: Uuid,
        skill_id: Uuid,
        payload: UpdateSessionSkillBindingRequest,
    ) -> CoreResult<SessionSkillsDetail> {
        self.store
            .update_session_skill_binding(session_id, skill_id, payload)
            .await
    }

    pub async fn apply_session_skill_preset(
        &self,
        session_id: Uuid,
        payload: ApplySkillPresetRequest,
    ) -> CoreResult<SessionSkillsDetail> {
        self.store
            .apply_session_skill_preset(session_id, payload.preset_id)
            .await
    }
}
