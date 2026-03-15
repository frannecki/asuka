use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::{domain::*, error::CoreResult};

use super::{
    helpers::{load_json_record, load_json_records, serialize_record, sqlite_error},
    store::SqliteStore,
    tables::agent_mcp_servers,
};

impl SqliteStore {
    pub(super) async fn list_mcp_servers_db(&self) -> CoreResult<Vec<McpServerRecord>> {
        let mut connection = self.open_connection()?;
        load_json_records(
            &mut connection,
            agent_mcp_servers::table
                .order(agent_mcp_servers::updated_at.desc())
                .select(agent_mcp_servers::data),
            "mcp server",
        )
    }

    pub(super) async fn create_mcp_server_db(
        &self,
        payload: CreateMcpServerRequest,
    ) -> CoreResult<McpServerRecord> {
        let server = McpServerRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            transport: payload.transport,
            command: payload.command,
            status: ResourceStatus::Active,
            capabilities: vec!["tools.call".into(), "resources.read".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut connection = self.open_connection()?;
        diesel::insert_into(agent_mcp_servers::table)
            .values((
                agent_mcp_servers::id.eq(server.id.to_string()),
                agent_mcp_servers::name.eq(server.name.clone()),
                agent_mcp_servers::updated_at.eq(server.updated_at.to_rfc3339()),
                agent_mcp_servers::data.eq(serialize_record(&server, "mcp server")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert mcp server", error))?;
        Ok(server)
    }

    pub(super) async fn get_mcp_server_db(&self, server_id: Uuid) -> CoreResult<McpServerRecord> {
        let mut connection = self.open_connection()?;
        load_json_record(
            &mut connection,
            agent_mcp_servers::table
                .filter(agent_mcp_servers::id.eq(server_id.to_string()))
                .select(agent_mcp_servers::data),
            "mcp server",
        )
    }

    pub(super) async fn test_mcp_server_db(&self, server_id: Uuid) -> CoreResult<TestResult> {
        let server = self.get_mcp_server_db(server_id).await?;
        Ok(TestResult {
            ok: true,
            message: format!(
                "{} is reachable in this prototype via {} transport.",
                server.name, server.transport
            ),
        })
    }

    pub(super) async fn get_mcp_capabilities_db(
        &self,
        server_id: Uuid,
    ) -> CoreResult<CapabilityEnvelope> {
        let server = self.get_mcp_server_db(server_id).await?;
        Ok(CapabilityEnvelope {
            capabilities: server.capabilities,
        })
    }
}
