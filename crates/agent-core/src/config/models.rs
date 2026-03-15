use std::{fs, path::Path};

use serde::Deserialize;
use uuid::Uuid;

use crate::domain::{ProviderModelRecord, ProviderType};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsConfig {
    pub(crate) providers: Vec<ConfiguredProvider>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfiguredProvider {
    pub(crate) provider_type: ProviderType,
    pub(crate) display_name: String,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key_env: Option<String>,
    pub(crate) default_model: Option<String>,
    pub(crate) models: Vec<ConfiguredModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ConfiguredModel {
    name: String,
    context_window: u32,
    supports_tools: bool,
    supports_embeddings: bool,
    capabilities: Vec<String>,
}

impl ConfiguredProvider {
    pub(crate) fn to_models(&self) -> Vec<ProviderModelRecord> {
        self.models
            .iter()
            .map(|model| ProviderModelRecord {
                id: Uuid::new_v5(
                    &Uuid::NAMESPACE_OID,
                    format!("provider-model:{}:{}", self.display_name, model.name).as_bytes(),
                ),
                model_name: model.name.clone(),
                context_window: model.context_window,
                supports_tools: model.supports_tools,
                supports_embeddings: model.supports_embeddings,
                capabilities: model.capabilities.clone(),
                is_default: self
                    .default_model
                    .as_ref()
                    .map(|default_model| default_model == &model.name)
                    .unwrap_or(false),
            })
            .collect()
    }
}

pub fn load_models_config(path: &Path) -> anyhow::Result<ModelsConfig> {
    let contents = fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}
