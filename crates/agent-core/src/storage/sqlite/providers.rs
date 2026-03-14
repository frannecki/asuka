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
    pub(super) async fn list_providers_db(&self) -> CoreResult<Vec<ProviderAccountRecord>> {
        let connection = self.open_connection()?;
        query_json_records(
            &connection,
            "SELECT data FROM agent_providers ORDER BY updated_at DESC",
            [],
            "provider",
        )
    }

    pub(super) async fn create_provider_db(
        &self,
        payload: CreateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        let provider = ProviderAccountRecord {
            id: Uuid::new_v4(),
            provider_type: payload.provider_type,
            display_name: payload.display_name,
            base_url: payload.base_url,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            models: Vec::new(),
        };

        let connection = self.open_connection()?;
        let data = serialize_record(&provider, "provider")?;
        connection
            .execute(
                r#"
                INSERT INTO agent_providers (id, display_name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![
                    provider.id.to_string(),
                    provider.display_name,
                    provider.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("insert provider", error))?;
        Ok(provider)
    }

    pub(super) async fn get_provider_db(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<ProviderAccountRecord> {
        let connection = self.open_connection()?;
        get_json_record_by_id(&connection, "agent_providers", provider_id, "provider")
    }

    pub(super) async fn update_provider_db(
        &self,
        provider_id: Uuid,
        payload: UpdateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        let connection = self.open_connection()?;
        let mut provider = get_json_record_by_id::<ProviderAccountRecord>(
            &connection,
            "agent_providers",
            provider_id,
            "provider",
        )?;
        if let Some(display_name) = payload.display_name {
            provider.display_name = display_name;
        }
        if let Some(base_url) = payload.base_url {
            provider.base_url = Some(base_url);
        }
        if let Some(status) = payload.status {
            provider.status = status;
        }
        provider.updated_at = Utc::now();
        update_named_row(
            &connection,
            "agent_providers",
            "display_name",
            &provider.display_name,
            provider.id,
            provider.updated_at.to_rfc3339(),
            &provider,
            "provider",
        )?;
        Ok(provider)
    }

    pub(super) async fn list_provider_models_db(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<Vec<ProviderModelRecord>> {
        Ok(self.get_provider_db(provider_id).await?.models)
    }

    pub(super) async fn replace_provider_models_db(
        &self,
        provider_id: Uuid,
        base_url: Option<String>,
        models: Vec<ProviderModelRecord>,
    ) -> CoreResult<ProviderAccountRecord> {
        let connection = self.open_connection()?;
        let mut provider = get_json_record_by_id::<ProviderAccountRecord>(
            &connection,
            "agent_providers",
            provider_id,
            "provider",
        )?;
        provider.base_url = base_url;
        provider.models = models;
        provider.updated_at = Utc::now();
        update_named_row(
            &connection,
            "agent_providers",
            "display_name",
            &provider.display_name,
            provider.id,
            provider.updated_at.to_rfc3339(),
            &provider,
            "provider",
        )?;
        Ok(provider)
    }
}
