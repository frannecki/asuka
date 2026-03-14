use uuid::Uuid;

use crate::{
    config::load_models_config,
    core::AgentCore,
    domain::{
        CreateProviderRequest, ProviderAccountRecord, ProviderModelRecord, TestResult,
        UpdateProviderRequest,
    },
    error::{CoreError, CoreResult},
};

impl AgentCore {
    pub async fn list_providers(&self) -> CoreResult<Vec<ProviderAccountRecord>> {
        self.store.list_providers().await
    }

    pub async fn create_provider(
        &self,
        payload: CreateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        self.store.create_provider(payload).await
    }

    pub async fn get_provider(&self, provider_id: Uuid) -> CoreResult<ProviderAccountRecord> {
        self.store.get_provider(provider_id).await
    }

    pub async fn update_provider(
        &self,
        provider_id: Uuid,
        payload: UpdateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        self.store.update_provider(provider_id, payload).await
    }

    pub async fn test_provider(&self, provider_id: Uuid) -> CoreResult<TestResult> {
        let provider = self.store.get_provider(provider_id).await?;
        let selection = (
            provider.display_name.clone(),
            provider.models.len(),
            self.find_configured_provider(&provider.provider_type, &provider.display_name)
                .and_then(|entry| entry.api_key_env.clone()),
        );

        let api_status = selection
            .2
            .map(|env_name| {
                if std::env::var(&env_name)
                    .ok()
                    .filter(|value| !value.is_empty())
                    .is_some()
                {
                    format!("credential env {env_name} is present")
                } else {
                    format!("credential env {env_name} is not set")
                }
            })
            .unwrap_or_else(|| "no credential env is configured".to_string());

        Ok(TestResult {
            ok: true,
            message: format!(
                "{} is registered with {} model(s); {}.",
                selection.0, selection.1, api_status
            ),
        })
    }

    pub async fn list_provider_models(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<Vec<ProviderModelRecord>> {
        self.store.list_provider_models(provider_id).await
    }

    pub async fn sync_provider_models(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<ProviderAccountRecord> {
        let config = load_models_config(&self.config_path).map_err(|error| {
            CoreError::new(500, format!("failed to load model config: {error}"))
        })?;
        let provider = self.store.get_provider(provider_id).await?;
        let configured_provider = config.providers.iter().find(|entry| {
            entry.provider_type == provider.provider_type
                && entry.display_name == provider.display_name
        });

        match configured_provider {
            Some(configured_provider) => {
                self.store
                    .replace_provider_models(
                        provider_id,
                        configured_provider.base_url.clone(),
                        configured_provider.to_models(),
                    )
                    .await
            }
            None => Ok(provider),
        }
    }
}
