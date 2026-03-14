use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::{domain::*, error::CoreResult};

use super::{
    helpers::{get_json_record_by_id, query_json_records, serialize_record, sqlite_error},
    store::SqliteStore,
};

impl SqliteStore {
    pub(super) async fn list_mcp_servers_db(&self) -> CoreResult<Vec<McpServerRecord>> {
        let connection = self.open_connection()?;
        query_json_records(
            &connection,
            "SELECT data FROM agent_mcp_servers ORDER BY updated_at DESC",
            [],
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

        let connection = self.open_connection()?;
        let data = serialize_record(&server, "mcp server")?;
        connection
            .execute(
                r#"
                INSERT INTO agent_mcp_servers (id, name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![
                    server.id.to_string(),
                    server.name,
                    server.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("insert mcp server", error))?;
        Ok(server)
    }

    pub(super) async fn get_mcp_server_db(&self, server_id: Uuid) -> CoreResult<McpServerRecord> {
        let connection = self.open_connection()?;
        get_json_record_by_id(&connection, "agent_mcp_servers", server_id, "mcp server")
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
