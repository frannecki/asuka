use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::{
    domain::{MemoryChunkRecord, MemoryDocumentRecord, RunRecord},
    error::{CoreError, CoreResult},
    memory::chunk_memory_document,
};

pub(super) fn insert_memory_document_and_chunks_sqlite(
    connection: &Connection,
    document: &MemoryDocumentRecord,
) -> CoreResult<MemoryDocumentRecord> {
    let mut document = document.clone();
    let chunks = chunk_memory_document(&document);
    document.chunk_count = chunks.len();

    let data = serialize_record(&document, "memory document")?;
    connection
        .execute(
            r#"
            INSERT INTO agent_memory_documents
                (id, namespace, source, title, created_at, updated_at, data)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                document.id.to_string(),
                document.namespace,
                document.source,
                document.title,
                document.created_at.to_rfc3339(),
                document.updated_at.to_rfc3339(),
                data
            ],
        )
        .map_err(|error| sqlite_error("insert memory document", error))?;
    insert_memory_chunks_sqlite(connection, &chunks)?;
    Ok(document)
}

pub(super) fn insert_memory_chunks_sqlite(
    connection: &Connection,
    chunks: &[MemoryChunkRecord],
) -> CoreResult<()> {
    for chunk in chunks {
        let data = serialize_record(chunk, "memory chunk")?;
        let keywords = serde_json::to_string(&chunk.keywords).map_err(|error| {
            CoreError::new(
                500,
                format!("failed to serialize memory chunk keywords for sqlite: {error}"),
            )
        })?;
        connection
            .execute(
                r#"
                INSERT INTO agent_memory_chunks (id, document_id, namespace, ordinal, keywords, data)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    chunk.id.to_string(),
                    chunk.document_id.to_string(),
                    chunk.namespace,
                    chunk.ordinal as i64,
                    keywords,
                    data
                ],
            )
            .map_err(|error| sqlite_error("insert memory chunk", error))?;
    }

    Ok(())
}

pub(super) fn ensure_row_exists(
    connection: &Connection,
    table: &str,
    id: Uuid,
    entity: &str,
) -> CoreResult<()> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    let exists: Option<i64> = connection
        .query_row(&sql, [id.to_string()], |row| row.get(0))
        .optional()
        .map_err(|error| sqlite_error(&format!("lookup {entity}"), error))?;
    if exists.is_none() {
        return Err(CoreError::not_found(entity));
    }
    Ok(())
}

pub(super) fn get_json_record_by_id<T>(
    connection: &Connection,
    table: &str,
    id: Uuid,
    entity: &str,
) -> CoreResult<T>
where
    T: DeserializeOwned,
{
    let sql = format!("SELECT data FROM {table} WHERE id = ?1");
    let data: Option<String> = connection
        .query_row(&sql, [id.to_string()], |row| row.get(0))
        .optional()
        .map_err(|error| sqlite_error(&format!("load {entity}"), error))?;
    let data = data.ok_or_else(|| CoreError::not_found(entity))?;
    deserialize_record(&data, entity)
}

pub(super) fn query_json_records<T, P>(
    connection: &Connection,
    sql: &str,
    params: P,
    entity: &str,
) -> CoreResult<Vec<T>>
where
    T: DeserializeOwned,
    P: rusqlite::Params,
{
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| sqlite_error(&format!("prepare {entity} query"), error))?;
    let mut rows = statement
        .query(params)
        .map_err(|error| sqlite_error(&format!("execute {entity} query"), error))?;

    let mut results = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_error(&format!("iterate {entity} rows"), error))?
    {
        let data: String = row
            .get(0)
            .map_err(|error| sqlite_error(&format!("read {entity} row"), error))?;
        results.push(deserialize_record(&data, entity)?);
    }
    Ok(results)
}

pub(super) fn update_json_row<T: Serialize>(
    connection: &Connection,
    table: &str,
    id: Uuid,
    updated_at: String,
    record: &T,
    entity: &str,
) -> CoreResult<()> {
    let data = serialize_record(record, entity)?;
    let sql = format!("UPDATE {table} SET updated_at = ?2, data = ?3 WHERE id = ?1");
    let updated = connection
        .execute(&sql, params![id.to_string(), updated_at, data])
        .map_err(|error| sqlite_error(&format!("update {entity}"), error))?;
    if updated == 0 {
        return Err(CoreError::not_found(entity));
    }
    Ok(())
}

pub(super) fn update_named_row<T: Serialize>(
    connection: &Connection,
    table: &str,
    named_column: &str,
    named_value: &str,
    id: Uuid,
    updated_at: String,
    record: &T,
    entity: &str,
) -> CoreResult<()> {
    let data = serialize_record(record, entity)?;
    let sql =
        format!("UPDATE {table} SET {named_column} = ?2, updated_at = ?3, data = ?4 WHERE id = ?1");
    let updated = connection
        .execute(&sql, params![id.to_string(), named_value, updated_at, data])
        .map_err(|error| sqlite_error(&format!("update {entity}"), error))?;
    if updated == 0 {
        return Err(CoreError::not_found(entity));
    }
    Ok(())
}

pub(super) fn update_run_row(connection: &Connection, run: &RunRecord) -> CoreResult<()> {
    let data = serialize_record(run, "run")?;
    let updated = connection
        .execute(
            r#"
            UPDATE agent_runs
            SET finished_at = ?2, data = ?3
            WHERE id = ?1
            "#,
            params![
                run.id.to_string(),
                run.finished_at.map(|value| value.to_rfc3339()),
                data
            ],
        )
        .map_err(|error| sqlite_error("update run", error))?;
    if updated == 0 {
        return Err(CoreError::not_found("run"));
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

pub(super) fn sqlite_error(action: &str, error: rusqlite::Error) -> CoreError {
    CoreError::new(500, format!("{action}: {error}"))
}
