use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ResourceStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRecord {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub status: ResourceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkillRequest {
    pub description: Option<String>,
    pub status: Option<ResourceStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionSkillPolicyMode {
    InheritDefault,
    Preset,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionSkillAvailability {
    Enabled,
    Disabled,
    Pinned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSkillPolicy {
    pub session_id: Uuid,
    pub mode: SessionSkillPolicyMode,
    pub preset_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl SessionSkillPolicy {
    pub fn default_for(session_id: Uuid) -> Self {
        Self {
            session_id,
            mode: SessionSkillPolicyMode::InheritDefault,
            preset_id: None,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSkillBinding {
    pub session_id: Uuid,
    pub skill_id: Uuid,
    pub availability: SessionSkillAvailability,
    pub order_index: i32,
    pub notes: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPreset {
    pub id: String,
    pub title: String,
    pub description: String,
    pub skill_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveSessionSkill {
    pub skill: SkillRecord,
    pub availability: SessionSkillAvailability,
    pub is_explicit: bool,
    pub is_preset: bool,
    pub is_pinned: bool,
    pub order_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSkillSummary {
    pub policy: SessionSkillPolicy,
    pub effective_skill_count: usize,
    pub pinned_skills: Vec<SkillRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSkillsDetail {
    pub session_id: Uuid,
    pub policy: SessionSkillPolicy,
    pub bindings: Vec<SessionSkillBinding>,
    pub effective_skills: Vec<EffectiveSessionSkill>,
    pub presets: Vec<SkillPreset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSkillBindingInput {
    pub skill_id: Uuid,
    pub availability: SessionSkillAvailability,
    pub order_index: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceSessionSkillsRequest {
    pub mode: SessionSkillPolicyMode,
    pub preset_id: Option<String>,
    #[serde(default)]
    pub bindings: Vec<SessionSkillBindingInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionSkillBindingRequest {
    pub availability: SessionSkillAvailability,
    pub order_index: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySkillPresetRequest {
    pub preset_id: String,
}

pub fn default_skill_presets() -> Vec<SkillPreset> {
    vec![
        SkillPreset {
            id: "minimal".to_string(),
            title: "Minimal".to_string(),
            description: "Keep the session focused with only orchestration essentials.".to_string(),
            skill_names: vec!["planning-skill".to_string()],
        },
        SkillPreset {
            id: "coding".to_string(),
            title: "Coding".to_string(),
            description: "Bias the session toward planning, filesystem work, and debugging."
                .to_string(),
            skill_names: vec![
                "planning-skill".to_string(),
                "filesystem-skill".to_string(),
                "debugging-skill".to_string(),
            ],
        },
        SkillPreset {
            id: "research".to_string(),
            title: "Research".to_string(),
            description: "Bias the session toward investigation and source-backed synthesis."
                .to_string(),
            skill_names: vec!["research-skill".to_string(), "planning-skill".to_string()],
        },
        SkillPreset {
            id: "agent-debugging".to_string(),
            title: "Agent Debugging".to_string(),
            description: "Keep agent inspection, filesystem, and debugging skills prominent."
                .to_string(),
            skill_names: vec![
                "planning-skill".to_string(),
                "debugging-skill".to_string(),
                "filesystem-skill".to_string(),
            ],
        },
    ]
}

pub fn resolve_session_skills(
    session_id: Uuid,
    policy: Option<SessionSkillPolicy>,
    bindings: Vec<SessionSkillBinding>,
    skills: Vec<SkillRecord>,
) -> SessionSkillsDetail {
    let policy = policy.unwrap_or_else(|| SessionSkillPolicy::default_for(session_id));
    let presets = default_skill_presets();
    let mut active_skills = skills
        .into_iter()
        .filter(|skill| matches!(skill.status, ResourceStatus::Active))
        .collect::<Vec<_>>();
    active_skills.sort_by(|left, right| left.name.cmp(&right.name));

    let preset_skill_names = if matches!(policy.mode, SessionSkillPolicyMode::Preset) {
        presets
            .iter()
            .find(|preset| Some(preset.id.as_str()) == policy.preset_id.as_deref())
            .map(|preset| preset.skill_names.clone())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut effective_skills = active_skills
        .iter()
        .enumerate()
        .filter_map(|(index, skill)| {
            let included = match policy.mode {
                SessionSkillPolicyMode::InheritDefault => true,
                SessionSkillPolicyMode::Custom => false,
                SessionSkillPolicyMode::Preset => {
                    preset_skill_names.iter().any(|name| name == &skill.name)
                }
            };

            included.then(|| EffectiveSessionSkill {
                skill: skill.clone(),
                availability: SessionSkillAvailability::Enabled,
                is_explicit: false,
                is_preset: matches!(policy.mode, SessionSkillPolicyMode::Preset),
                is_pinned: false,
                order_index: index as i32,
            })
        })
        .collect::<Vec<_>>();

    let mut ordered_bindings = bindings;
    ordered_bindings.sort_by(|left, right| {
        left.order_index
            .cmp(&right.order_index)
            .then_with(|| left.skill_id.cmp(&right.skill_id))
    });

    for binding in &ordered_bindings {
        let Some(skill) = active_skills
            .iter()
            .find(|candidate| candidate.id == binding.skill_id)
        else {
            continue;
        };

        let existing_index = effective_skills
            .iter()
            .position(|candidate| candidate.skill.id == binding.skill_id);

        match binding.availability {
            SessionSkillAvailability::Disabled => {
                if let Some(existing_index) = existing_index {
                    effective_skills.remove(existing_index);
                }
            }
            SessionSkillAvailability::Enabled | SessionSkillAvailability::Pinned => {
                let entry = EffectiveSessionSkill {
                    skill: skill.clone(),
                    availability: binding.availability.clone(),
                    is_explicit: true,
                    is_preset: false,
                    is_pinned: matches!(binding.availability, SessionSkillAvailability::Pinned),
                    order_index: binding.order_index,
                };

                if let Some(existing_index) = existing_index {
                    effective_skills[existing_index] = entry;
                } else {
                    effective_skills.push(entry);
                }
            }
        }
    }

    effective_skills.sort_by(|left, right| {
        right
            .is_pinned
            .cmp(&left.is_pinned)
            .then_with(|| left.order_index.cmp(&right.order_index))
            .then_with(|| left.skill.name.cmp(&right.skill.name))
    });

    SessionSkillsDetail {
        session_id,
        policy: policy.clone(),
        bindings: ordered_bindings,
        effective_skills: effective_skills.clone(),
        presets,
    }
}

pub fn summarize_session_skills(detail: &SessionSkillsDetail) -> SessionSkillSummary {
    SessionSkillSummary {
        policy: detail.policy.clone(),
        effective_skill_count: detail.effective_skills.len(),
        pinned_skills: detail
            .effective_skills
            .iter()
            .filter(|entry| entry.is_pinned)
            .map(|entry| entry.skill.clone())
            .collect(),
    }
}
