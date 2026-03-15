use diesel::{prelude::*, query_dsl::LoadQuery, sqlite::SqliteConnection, RunQueryDsl};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    domain::{MemoryChunkRecord, MemoryDocumentRecord},
    error::{CoreError, CoreResult},
    memory::chunk_memory_document,
};

use super::tables::{agent_memory_chunks, agent_memory_documents};

pub(super) fn insert_memory_document_and_chunks_sqlite(
    connection: &mut SqliteConnection,
    document: &MemoryDocumentRecord,
) -> CoreResult<MemoryDocumentRecord> {
    let mut document = document.clone();
    let chunks = chunk_memory_document(&document);
    document.chunk_count = chunks.len();

    diesel::insert_into(agent_memory_documents::table)
        .values((
            agent_memory_documents::id.eq(document.id.to_string()),
            agent_memory_documents::namespace.eq(document.namespace.clone()),
            agent_memory_documents::source.eq(document.source.clone()),
            agent_memory_documents::title.eq(document.title.clone()),
            agent_memory_documents::created_at.eq(document.created_at.to_rfc3339()),
            agent_memory_documents::updated_at.eq(document.updated_at.to_rfc3339()),
            agent_memory_documents::data.eq(serialize_record(&document, "memory document")?),
        ))
        .execute(connection)
        .map_err(|error| sqlite_error("insert memory document", error))?;
    insert_memory_chunks_sqlite(connection, &chunks)?;
    Ok(document)
}

pub(super) fn insert_memory_chunks_sqlite(
    connection: &mut SqliteConnection,
    chunks: &[MemoryChunkRecord],
) -> CoreResult<()> {
    for chunk in chunks {
        let keywords = serde_json::to_string(&chunk.keywords).map_err(|error| {
            CoreError::new(
                500,
                format!("failed to serialize memory chunk keywords for sqlite: {error}"),
            )
        })?;
        diesel::insert_into(agent_memory_chunks::table)
            .values((
                agent_memory_chunks::id.eq(chunk.id.to_string()),
                agent_memory_chunks::document_id.eq(chunk.document_id.to_string()),
                agent_memory_chunks::namespace.eq(chunk.namespace.clone()),
                agent_memory_chunks::ordinal.eq(chunk.ordinal as i64),
                agent_memory_chunks::keywords.eq(keywords),
                agent_memory_chunks::data.eq(serialize_record(chunk, "memory chunk")?),
            ))
            .execute(connection)
            .map_err(|error| sqlite_error("insert memory chunk", error))?;
    }

    Ok(())
}

pub(super) fn load_json_records<'query, T, Q>(
    connection: &mut SqliteConnection,
    query: Q,
    entity: &str,
) -> CoreResult<Vec<T>>
where
    T: DeserializeOwned,
    Q: LoadQuery<'query, SqliteConnection, String>,
{
    let rows = query
        .load::<String>(connection)
        .map_err(|error| sqlite_error(&format!("load {entity} rows"), error))?;
    deserialize_records(rows, entity)
}

pub(super) fn load_optional_json_record<'query, T, Q>(
    connection: &mut SqliteConnection,
    query: Q,
    entity: &str,
) -> CoreResult<Option<T>>
where
    T: DeserializeOwned,
    Q: LoadQuery<'query, SqliteConnection, String>,
{
    let rows = load_json_records(connection, query, entity)?;
    Ok(rows.into_iter().next())
}

pub(super) fn load_json_record<'query, T, Q>(
    connection: &mut SqliteConnection,
    query: Q,
    entity: &str,
) -> CoreResult<T>
where
    T: DeserializeOwned,
    Q: LoadQuery<'query, SqliteConnection, String>,
{
    load_optional_json_record(connection, query, entity)?
        .ok_or_else(|| CoreError::not_found(entity))
}

pub(super) fn expect_changed(changed: usize, entity: &str) -> CoreResult<()> {
    if changed == 0 {
        return Err(CoreError::not_found(entity));
    }
    Ok(())
}

pub(super) fn serialize_record<T: Serialize>(record: &T, entity: &str) -> CoreResult<String> {
    serde_json::to_string(record).map_err(|error| {
        CoreError::new(
            500,
            format!("failed to serialize {entity} record for sqlite: {error}"),
        )
    })
}

pub(super) fn deserialize_record<T: DeserializeOwned>(data: &str, entity: &str) -> CoreResult<T> {
    serde_json::from_str(data).map_err(|error| {
        CoreError::new(
            500,
            format!("failed to deserialize {entity} record from sqlite: {error}"),
        )
    })
}

pub(super) fn deserialize_records<T: DeserializeOwned>(
    rows: Vec<String>,
    entity: &str,
) -> CoreResult<Vec<T>> {
    rows.into_iter()
        .map(|row| deserialize_record(&row, entity))
        .collect()
}

pub(super) fn sqlite_error(action: &str, error: impl std::fmt::Display) -> CoreError {
    CoreError::new(500, format!("{action}: {error}"))
}
