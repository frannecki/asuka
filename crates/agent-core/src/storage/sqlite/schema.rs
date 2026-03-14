use rusqlite::{params, Connection, OptionalExtension};

use crate::{config::ModelsConfig, error::CoreResult, storage::StoreState};

use super::helpers::{insert_memory_document_and_chunks_sqlite, serialize_record, sqlite_error};

pub(super) fn init_schema(connection: &Connection) -> CoreResult<()> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS agent_sessions (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_updated_at
                ON agent_sessions(updated_at DESC);

            CREATE TABLE IF NOT EXISTS agent_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
                run_id TEXT NULL,
                created_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_messages_session_created
                ON agent_messages(session_id, created_at);

            CREATE TABLE IF NOT EXISTS agent_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
                started_at TEXT NOT NULL,
                finished_at TEXT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_runs_session_started
                ON agent_runs(session_id, started_at DESC);

            CREATE TABLE IF NOT EXISTS agent_skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_subagents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_providers (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL UNIQUE,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_memory_documents (
                id TEXT PRIMARY KEY,
                namespace TEXT NOT NULL,
                source TEXT NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memory_documents_identity
                ON agent_memory_documents(namespace, source, title);
            CREATE INDEX IF NOT EXISTS idx_agent_memory_documents_namespace_updated
                ON agent_memory_documents(namespace, updated_at DESC);

            CREATE TABLE IF NOT EXISTS agent_memory_chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL REFERENCES agent_memory_documents(id) ON DELETE CASCADE,
                namespace TEXT NOT NULL,
                ordinal INTEGER NOT NULL,
                keywords TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memory_chunks_identity
                ON agent_memory_chunks(document_id, ordinal);
            CREATE INDEX IF NOT EXISTS idx_agent_memory_chunks_namespace_ordinal
                ON agent_memory_chunks(namespace, ordinal);

            CREATE TABLE IF NOT EXISTS agent_mcp_servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            "#,
        )
        .map_err(|error| sqlite_error("initialize sqlite schema", error))?;
    Ok(())
}

pub(super) fn seed_defaults(connection: &mut Connection, config: &ModelsConfig) -> CoreResult<()> {
    let seeded = StoreState::seeded(config);
    let transaction = connection
        .transaction()
        .map_err(|error| sqlite_error("begin sqlite seed transaction", error))?;

    for provider in seeded.providers.values() {
        let data = serialize_record(provider, "provider")?;
        transaction
            .execute(
                r#"
                INSERT INTO agent_providers (id, display_name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(display_name) DO UPDATE SET
                    updated_at = excluded.updated_at,
                    data = excluded.data
                "#,
                params![
                    provider.id.to_string(),
                    provider.display_name,
                    provider.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("seed provider", error))?;
    }

    for skill in seeded.skills.values() {
        let data = serialize_record(skill, "skill")?;
        transaction
            .execute(
                r#"
                INSERT INTO agent_skills (id, name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(name) DO UPDATE SET
                    updated_at = excluded.updated_at,
                    data = excluded.data
                "#,
                params![
                    skill.id.to_string(),
                    skill.name,
                    skill.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("seed skill", error))?;
    }

    for subagent in seeded.subagents.values() {
        let data = serialize_record(subagent, "subagent")?;
        transaction
            .execute(
                r#"
                INSERT INTO agent_subagents (id, name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(name) DO UPDATE SET
                    updated_at = excluded.updated_at,
                    data = excluded.data
                "#,
                params![
                    subagent.id.to_string(),
                    subagent.name,
                    subagent.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("seed subagent", error))?;
    }

    for document in seeded.memory_documents.values() {
        let existing: Option<String> = transaction
            .query_row(
                r#"
                SELECT id
                FROM agent_memory_documents
                WHERE namespace = ?1 AND source = ?2 AND title = ?3
                "#,
                params![document.namespace, document.source, document.title],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| sqlite_error("check seeded memory document", error))?;
        if existing.is_none() {
            insert_memory_document_and_chunks_sqlite(&transaction, document)?;
        }
    }

    for server in seeded.mcp_servers.values() {
        let data = serialize_record(server, "mcp server")?;
        transaction
            .execute(
                r#"
                INSERT INTO agent_mcp_servers (id, name, updated_at, data)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(name) DO UPDATE SET
                    updated_at = excluded.updated_at,
                    data = excluded.data
                "#,
                params![
                    server.id.to_string(),
                    server.name,
                    server.updated_at.to_rfc3339(),
                    data
                ],
            )
            .map_err(|error| sqlite_error("seed mcp server", error))?;
    }

    let session_count: i64 = transaction
        .query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))
        .map_err(|error| sqlite_error("count sqlite sessions", error))?;
    if session_count == 0 {
        for session in seeded.sessions.values() {
            let data = serialize_record(session, "session")?;
            transaction
                .execute(
                    r#"
                    INSERT INTO agent_sessions (id, created_at, updated_at, data)
                    VALUES (?1, ?2, ?3, ?4)
                    "#,
                    params![
                        session.id.to_string(),
                        session.created_at.to_rfc3339(),
                        session.updated_at.to_rfc3339(),
                        data
                    ],
                )
                .map_err(|error| sqlite_error("seed session", error))?;
        }

        for messages in seeded.messages.values() {
            for message in messages {
                let data = serialize_record(message, "message")?;
                transaction
                    .execute(
                        r#"
                        INSERT INTO agent_messages (id, session_id, run_id, created_at, data)
                        VALUES (?1, ?2, ?3, ?4, ?5)
                        "#,
                        params![
                            message.id.to_string(),
                            message.session_id.to_string(),
                            message.run_id.map(|value| value.to_string()),
                            message.created_at.to_rfc3339(),
                            data
                        ],
                    )
                    .map_err(|error| sqlite_error("seed message", error))?;
            }
        }
    }

    transaction
        .commit()
        .map_err(|error| sqlite_error("commit sqlite seed transaction", error))?;
    Ok(())
}
