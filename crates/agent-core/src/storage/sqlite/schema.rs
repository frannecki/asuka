use diesel::{
    connection::SimpleConnection,
    dsl::{count_star, exists, select},
    prelude::*,
    sql_query,
    sql_types::Text,
    sqlite::SqliteConnection,
    upsert::excluded,
    OptionalExtension, QueryableByName,
};

use crate::{
    config::ModelsConfig,
    error::{CoreError, CoreResult},
    storage::StoreState,
};

use super::{
    helpers::{insert_memory_document_and_chunks_sqlite, serialize_record, sqlite_error},
    tables::{
        agent_mcp_servers, agent_memory_documents, agent_messages, agent_providers,
        agent_session_skill_bindings, agent_sessions, agent_skills, agent_subagents,
    },
};

pub(super) fn init_schema(connection: &mut SqliteConnection) -> CoreResult<()> {
    connection
        .batch_execute(
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
                task_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_runs_session_started
                ON agent_runs(session_id, started_at DESC);
            CREATE TABLE IF NOT EXISTS agent_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
                session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                created_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_run_events_run_sequence
                ON agent_run_events(run_id, sequence);
            CREATE INDEX IF NOT EXISTS idx_agent_run_events_session_sequence
                ON agent_run_events(session_id, sequence);
            CREATE TABLE IF NOT EXISTS agent_tasks (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_tasks_session_updated
                ON agent_tasks(session_id, updated_at DESC);

            CREATE TABLE IF NOT EXISTS agent_artifacts (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
                task_id TEXT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
                run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_artifacts_run_path
                ON agent_artifacts(run_id, path);
            CREATE INDEX IF NOT EXISTS idx_agent_artifacts_session_updated
                ON agent_artifacts(session_id, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_agent_artifacts_task_updated
                ON agent_artifacts(task_id, updated_at DESC);

            CREATE TABLE IF NOT EXISTS agent_plans (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
                version INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_plans_task_created
                ON agent_plans(task_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS agent_plan_steps (
                id TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL REFERENCES agent_plans(id) ON DELETE CASCADE,
                ordinal INTEGER NOT NULL,
                status TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_plan_steps_plan_ordinal
                ON agent_plan_steps(plan_id, ordinal);

            CREATE TABLE IF NOT EXISTS agent_run_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
                task_id TEXT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NULL,
                data TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_run_steps_run_sequence
                ON agent_run_steps(run_id, sequence);

            CREATE TABLE IF NOT EXISTS agent_tool_invocations (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
                run_step_id TEXT NOT NULL REFERENCES agent_run_steps(id) ON DELETE CASCADE,
                tool_name TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_tool_invocations_run_started
                ON agent_tool_invocations(run_id, started_at);

            CREATE TABLE IF NOT EXISTS agent_skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_session_skill_policies (
                session_id TEXT PRIMARY KEY REFERENCES agent_sessions(id) ON DELETE CASCADE,
                updated_at TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_session_skill_bindings (
                session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
                skill_id TEXT NOT NULL REFERENCES agent_skills(id) ON DELETE CASCADE,
                updated_at TEXT NOT NULL,
                order_index INTEGER NOT NULL,
                data TEXT NOT NULL,
                PRIMARY KEY(session_id, skill_id)
            );
            CREATE INDEX IF NOT EXISTS idx_agent_session_skill_bindings_session_order
                ON agent_session_skill_bindings(session_id, order_index, updated_at DESC);

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
    ensure_column(connection, "agent_runs", "task_id", "TEXT")?;
    connection
        .batch_execute(
            r#"
            CREATE INDEX IF NOT EXISTS idx_agent_runs_task_started
                ON agent_runs(task_id, started_at DESC);
            "#,
        )
        .map_err(|error| sqlite_error("initialize sqlite run task index", error))?;
    Ok(())
}

pub(super) fn seed_defaults(
    connection: &mut SqliteConnection,
    config: &ModelsConfig,
) -> CoreResult<()> {
    let seeded = StoreState::seeded(config);
    connection.transaction::<_, CoreError, _>(|transaction| {
        for provider in seeded.providers.values() {
            diesel::insert_into(agent_providers::table)
                .values((
                    agent_providers::id.eq(provider.id.to_string()),
                    agent_providers::display_name.eq(provider.display_name.clone()),
                    agent_providers::updated_at.eq(provider.updated_at.to_rfc3339()),
                    agent_providers::data.eq(serialize_record(provider, "provider")?),
                ))
                .on_conflict(agent_providers::display_name)
                .do_update()
                .set((
                    agent_providers::updated_at.eq(excluded(agent_providers::updated_at)),
                    agent_providers::data.eq(excluded(agent_providers::data)),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("seed provider", error))?;
        }

        for skill in seeded.skills.values() {
            diesel::insert_into(agent_skills::table)
                .values((
                    agent_skills::id.eq(skill.id.to_string()),
                    agent_skills::name.eq(skill.name.clone()),
                    agent_skills::updated_at.eq(skill.updated_at.to_rfc3339()),
                    agent_skills::data.eq(serialize_record(skill, "skill")?),
                ))
                .on_conflict(agent_skills::name)
                .do_update()
                .set((
                    agent_skills::updated_at.eq(excluded(agent_skills::updated_at)),
                    agent_skills::data.eq(excluded(agent_skills::data)),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("seed skill", error))?;
        }
        repair_seeded_skill_ids(transaction, &seeded)?;

        for subagent in seeded.subagents.values() {
            diesel::insert_into(agent_subagents::table)
                .values((
                    agent_subagents::id.eq(subagent.id.to_string()),
                    agent_subagents::name.eq(subagent.name.clone()),
                    agent_subagents::updated_at.eq(subagent.updated_at.to_rfc3339()),
                    agent_subagents::data.eq(serialize_record(subagent, "subagent")?),
                ))
                .on_conflict(agent_subagents::name)
                .do_update()
                .set((
                    agent_subagents::updated_at.eq(excluded(agent_subagents::updated_at)),
                    agent_subagents::data.eq(excluded(agent_subagents::data)),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("seed subagent", error))?;
        }

        for document in seeded.memory_documents.values() {
            let exists = select(exists(
                agent_memory_documents::table.filter(
                    agent_memory_documents::namespace
                        .eq(document.namespace.clone())
                        .and(agent_memory_documents::source.eq(document.source.clone()))
                        .and(agent_memory_documents::title.eq(document.title.clone())),
                ),
            ))
            .get_result::<bool>(transaction)
            .map_err(|error| sqlite_error("check seeded memory document", error))?;
            if !exists {
                insert_memory_document_and_chunks_sqlite(transaction, document)?;
            }
        }

        for server in seeded.mcp_servers.values() {
            diesel::insert_into(agent_mcp_servers::table)
                .values((
                    agent_mcp_servers::id.eq(server.id.to_string()),
                    agent_mcp_servers::name.eq(server.name.clone()),
                    agent_mcp_servers::updated_at.eq(server.updated_at.to_rfc3339()),
                    agent_mcp_servers::data.eq(serialize_record(server, "mcp server")?),
                ))
                .on_conflict(agent_mcp_servers::name)
                .do_update()
                .set((
                    agent_mcp_servers::updated_at.eq(excluded(agent_mcp_servers::updated_at)),
                    agent_mcp_servers::data.eq(excluded(agent_mcp_servers::data)),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("seed mcp server", error))?;
        }

        let session_count = agent_sessions::table
            .select(count_star())
            .first::<i64>(transaction)
            .map_err(|error| sqlite_error("count sqlite sessions", error))?;
        if session_count == 0 {
            for session in seeded.sessions.values() {
                diesel::insert_into(agent_sessions::table)
                    .values((
                        agent_sessions::id.eq(session.id.to_string()),
                        agent_sessions::created_at.eq(session.created_at.to_rfc3339()),
                        agent_sessions::updated_at.eq(session.updated_at.to_rfc3339()),
                        agent_sessions::data.eq(serialize_record(session, "session")?),
                    ))
                    .execute(transaction)
                    .map_err(|error| sqlite_error("seed session", error))?;
            }

            for messages in seeded.messages.values() {
                for message in messages {
                    diesel::insert_into(agent_messages::table)
                        .values((
                            agent_messages::id.eq(message.id.to_string()),
                            agent_messages::session_id.eq(message.session_id.to_string()),
                            agent_messages::run_id
                                .eq(message.run_id.map(|value| value.to_string())),
                            agent_messages::created_at.eq(message.created_at.to_rfc3339()),
                            agent_messages::data.eq(serialize_record(message, "message")?),
                        ))
                        .execute(transaction)
                        .map_err(|error| sqlite_error("seed message", error))?;
                }
            }
        }

        Ok(())
    })?;
    Ok(())
}

fn repair_seeded_skill_ids(
    transaction: &mut SqliteConnection,
    seeded: &StoreState,
) -> CoreResult<()> {
    transaction
        .batch_execute("PRAGMA defer_foreign_keys = ON;")
        .map_err(|error| sqlite_error("defer sqlite foreign keys for skill repair", error))?;

    for skill in seeded.skills.values() {
        let existing_id = agent_skills::table
            .filter(agent_skills::name.eq(skill.name.clone()))
            .select(agent_skills::id)
            .first::<String>(transaction)
            .optional()
            .map_err(|error| sqlite_error("load seeded skill id for repair", error))?;
        let Some(existing_id) = existing_id else {
            continue;
        };

        let canonical_id = skill.id.to_string();
        if existing_id == canonical_id {
            continue;
        }

        diesel::update(
            agent_session_skill_bindings::table
                .filter(agent_session_skill_bindings::skill_id.eq(existing_id.clone())),
        )
        .set(agent_session_skill_bindings::skill_id.eq(canonical_id.clone()))
        .execute(transaction)
        .map_err(|error| sqlite_error("repair session skill binding ids", error))?;
        diesel::update(agent_skills::table.filter(agent_skills::name.eq(skill.name.clone())))
            .set((
                agent_skills::id.eq(canonical_id),
                agent_skills::updated_at.eq(skill.updated_at.to_rfc3339()),
                agent_skills::data.eq(serialize_record(skill, "skill")?),
            ))
            .execute(transaction)
            .map_err(|error| sqlite_error("repair seeded skill ids", error))?;
    }

    Ok(())
}

fn ensure_column(
    connection: &mut SqliteConnection,
    table: &str,
    column: &str,
    definition: &str,
) -> CoreResult<()> {
    #[derive(QueryableByName)]
    struct TableInfoRow {
        #[diesel(sql_type = Text)]
        name: String,
    }

    let rows = sql_query(format!("SELECT name FROM pragma_table_info('{table}')"))
        .load::<TableInfoRow>(connection)
        .map_err(|error| sqlite_error("query table info pragma", error))?;
    if rows.into_iter().any(|row| row.name == column) {
        return Ok(());
    }

    connection
        .batch_execute(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))
        .map_err(|error| sqlite_error(&format!("add column {column} to {table}"), error))?;
    Ok(())
}
