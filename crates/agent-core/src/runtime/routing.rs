use crate::{
    config::{load_models_config, ConfiguredProvider},
    core::AgentCore,
    domain::{ProviderAccountRecord, ProviderType, ResourceStatus},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct ProviderSelection {
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_type: ProviderType,
    pub model_name: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
}

impl AgentCore {
    pub(crate) fn select_provider_model(
        &self,
        providers: &[ProviderAccountRecord],
    ) -> Option<ProviderSelection> {
        let config = load_models_config(&self.config_path).ok()?;

        for configured_provider in &config.providers {
            let provider = match providers.iter().find(|provider| {
                provider.status == ResourceStatus::Active
                    && provider.provider_type == configured_provider.provider_type
                    && provider.display_name == configured_provider.display_name
            }) {
                Some(provider) => provider,
                None => continue,
            };

            if let Some(api_key_env) = &configured_provider.api_key_env {
                if std::env::var(api_key_env)
                    .ok()
                    .filter(|value| !value.is_empty())
                    .is_none()
                {
                    continue;
                }
            }

            let model_name = match configured_provider
                .default_model
                .clone()
                .or_else(|| {
                    provider
                        .models
                        .iter()
                        .find(|model| model.is_default)
                        .map(|model| model.model_name.clone())
                })
                .or_else(|| {
                    provider
                        .models
                        .first()
                        .map(|model| model.model_name.clone())
                }) {
                Some(model_name) => model_name,
                None => continue,
            };

            return Some(ProviderSelection {
                provider_id: provider.id,
                provider_name: provider.display_name.clone(),
                provider_type: provider.provider_type.clone(),
                model_name,
                base_url: provider
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string()),
                api_key_env: configured_provider.api_key_env.clone(),
            });
        }

        None
    }

    pub(crate) fn find_configured_provider(
        &self,
        provider_type: &ProviderType,
        display_name: &str,
    ) -> Option<ConfiguredProvider> {
        load_models_config(&self.config_path)
            .ok()?
            .providers
            .into_iter()
            .find(|provider| {
                provider.provider_type == *provider_type && provider.display_name == display_name
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{
        create_test_core, moonshot_provider_config_toml, multi_provider_config_toml,
        runtime_test_lock, EnvVarGuard,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn select_provider_model_prefers_first_available_configured_provider() {
        let _lock = runtime_test_lock().lock().expect("lock runtime test");
        let _missing_openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let core = create_test_core(multi_provider_config_toml());

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        assert_eq!(selection.provider_name, "OpenAI");
        assert_eq!(selection.model_name, "gpt-4.1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn select_provider_model_uses_openrouter_when_api_key_is_present() {
        let _lock = runtime_test_lock().lock().expect("lock runtime test");
        let _openrouter_key = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let core = create_test_core(multi_provider_config_toml());

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        assert_eq!(selection.provider_name, "OpenRouter");
        assert_eq!(
            selection.model_name,
            "nvidia/nemotron-3-super-120b-a12b:free"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn select_provider_model_uses_moonshot_when_api_key_is_present() {
        let _lock = runtime_test_lock().lock().expect("lock runtime test");
        let _moonshot_key = EnvVarGuard::set("MOONSHOT_API_KEY", "test-key");
        let core = create_test_core(moonshot_provider_config_toml());

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");

        assert_eq!(selection.provider_name, "Moonshot");
        assert_eq!(selection.model_name, "kimi-k2.5");
    }
}
