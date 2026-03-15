use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::{domain::*, error::CoreResult};

use super::{
    helpers::{
        expect_changed, load_json_record, load_json_records, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::agent_providers,
};

impl SqliteStore {
    pub(super) async fn list_providers_db(&self) -> CoreResult<Vec<ProviderAccountRecord>> {
        let mut connection = self.open_connection()?;
        load_json_records(
            &mut connection,
            agent_providers::table
                .order(agent_providers::updated_at.desc())
                .select(agent_providers::data),
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

        let mut connection = self.open_connection()?;
        diesel::insert_into(agent_providers::table)
            .values((
                agent_providers::id.eq(provider.id.to_string()),
                agent_providers::display_name.eq(provider.display_name.clone()),
                agent_providers::updated_at.eq(provider.updated_at.to_rfc3339()),
                agent_providers::data.eq(serialize_record(&provider, "provider")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert provider", error))?;
        Ok(provider)
    }

    pub(super) async fn get_provider_db(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<ProviderAccountRecord> {
        let mut connection = self.open_connection()?;
        load_json_record(
            &mut connection,
            agent_providers::table
                .filter(agent_providers::id.eq(provider_id.to_string()))
                .select(agent_providers::data),
            "provider",
        )
    }

    pub(super) async fn update_provider_db(
        &self,
        provider_id: Uuid,
        payload: UpdateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        let mut connection = self.open_connection()?;
        let mut provider = load_json_record::<ProviderAccountRecord, _>(
            &mut connection,
            agent_providers::table
                .filter(agent_providers::id.eq(provider_id.to_string()))
                .select(agent_providers::data),
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

        let updated = diesel::update(
            agent_providers::table.filter(agent_providers::id.eq(provider.id.to_string())),
        )
        .set((
            agent_providers::display_name.eq(provider.display_name.clone()),
            agent_providers::updated_at.eq(provider.updated_at.to_rfc3339()),
            agent_providers::data.eq(serialize_record(&provider, "provider")?),
        ))
        .execute(&mut connection)
        .map_err(|error| sqlite_error("update provider", error))?;
        expect_changed(updated, "provider")?;
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
        let mut connection = self.open_connection()?;
        let mut provider = load_json_record::<ProviderAccountRecord, _>(
            &mut connection,
            agent_providers::table
                .filter(agent_providers::id.eq(provider_id.to_string()))
                .select(agent_providers::data),
            "provider",
        )?;
        provider.base_url = base_url;
        provider.models = models;
        provider.updated_at = Utc::now();

        let updated = diesel::update(
            agent_providers::table.filter(agent_providers::id.eq(provider.id.to_string())),
        )
        .set((
            agent_providers::display_name.eq(provider.display_name.clone()),
            agent_providers::updated_at.eq(provider.updated_at.to_rfc3339()),
            agent_providers::data.eq(serialize_record(&provider, "provider")?),
        ))
        .execute(&mut connection)
        .map_err(|error| sqlite_error("replace provider models", error))?;
        expect_changed(updated, "provider")?;
        Ok(provider)
    }
}
