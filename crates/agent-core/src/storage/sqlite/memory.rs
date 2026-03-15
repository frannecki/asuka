use chrono::Utc;
use diesel::prelude::*;
use tracing::warn;
use uuid::Uuid;

use crate::{
    domain::*,
    error::{CoreError, CoreResult},
    memory::{
        chroma_records_for_document, chunk_memory_document, search_memory_hits, summarize_text,
        MemoryCorpus,
    },
};

use super::{
    helpers::{
        expect_changed, insert_memory_chunks_sqlite, insert_memory_document_and_chunks_sqlite,
        load_json_record, load_json_records, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::{agent_memory_chunks, agent_memory_documents},
};

impl SqliteStore {
    pub(super) async fn write_run_memory_note_db(
        &self,
        session_id: Uuid,
        user_content: &str,
        response: &str,
    ) -> CoreResult<MemoryDocumentRecord> {
        let document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: format!("Run note {}", Utc::now().format("%Y-%m-%d %H:%M:%S")),
            namespace: "session".to_string(),
            source: "run-summary".to_string(),
            memory_scope: MemoryScope::Session,
            owner_session_id: Some(session_id),
            owner_task_id: None,
            is_pinned: false,
            content: format!("User request: {user_content}\n\nAssistant response: {response}"),
            summary: summarize_text(response, 24),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let document = {
            let mut connection = self.open_connection()?;
            connection.transaction::<_, CoreError, _>(|transaction| {
                insert_memory_document_and_chunks_sqlite(transaction, &document)
            })?
        };

        if let Some(chroma) = &self.chroma {
            let records = chroma_records_for_document(&document);
            if let Err(error) = chroma.upsert_records(records).await {
                warn!("failed to add run note to chroma: {}", error.message);
            }
        }

        Ok(document)
    }

    pub(super) async fn list_memory_documents_db(&self) -> CoreResult<Vec<MemoryDocumentRecord>> {
        let mut connection = self.open_connection()?;
        load_json_records(
            &mut connection,
            agent_memory_documents::table
                .order(agent_memory_documents::updated_at.desc())
                .select(agent_memory_documents::data),
            "memory document",
        )
    }

    pub(super) async fn create_memory_document_db(
        &self,
        payload: CreateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        if payload.title.trim().is_empty() || payload.content.trim().is_empty() {
            return Err(CoreError::bad_request(
                "memory documents require both title and content",
            ));
        }

        let namespace = payload
            .namespace
            .unwrap_or_else(|| "global".to_string())
            .trim()
            .to_string();
        let source = payload
            .source
            .unwrap_or_else(|| "manual".to_string())
            .trim()
            .to_string();
        let document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: payload.title.trim().to_string(),
            namespace: namespace.clone(),
            source,
            memory_scope: payload
                .memory_scope
                .unwrap_or_else(|| default_memory_scope_from_namespace(Some(namespace.as_str()))),
            owner_session_id: payload.owner_session_id,
            owner_task_id: payload.owner_task_id,
            is_pinned: payload.is_pinned.unwrap_or(false),
            content: payload.content.trim().to_string(),
            summary: summarize_text(payload.content.trim(), 20),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        validate_memory_document_scope(&document)?;

        let document = {
            let mut connection = self.open_connection()?;
            connection.transaction::<_, CoreError, _>(|transaction| {
                insert_memory_document_and_chunks_sqlite(transaction, &document)
            })?
        };

        if let Some(chroma) = &self.chroma {
            let records = chroma_records_for_document(&document);
            if let Err(error) = chroma.upsert_records(records).await {
                warn!(
                    "failed to index memory document into chroma: {}",
                    error.message
                );
            }
        }

        Ok(document)
    }

    pub(super) async fn update_memory_document_db(
        &self,
        document_id: Uuid,
        payload: UpdateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        let document = {
            let mut connection = self.open_connection()?;
            connection.transaction::<_, CoreError, _>(|transaction| {
                let mut document = load_json_record::<MemoryDocumentRecord, _>(
                    transaction,
                    agent_memory_documents::table
                        .filter(agent_memory_documents::id.eq(document_id.to_string()))
                        .select(agent_memory_documents::data),
                    "memory document",
                )?;
                if let Some(title) = payload.title.clone() {
                    document.title = title.trim().to_string();
                }
                if let Some(namespace) = payload.namespace.clone() {
                    document.namespace = namespace.trim().to_string();
                }
                if let Some(memory_scope) = payload.memory_scope.clone() {
                    document.memory_scope = memory_scope;
                }
                if let Some(owner_session_id) = payload.owner_session_id {
                    document.owner_session_id = Some(owner_session_id);
                }
                if !matches!(document.memory_scope, MemoryScope::Session) {
                    document.owner_session_id = None;
                }
                if let Some(is_pinned) = payload.is_pinned {
                    document.is_pinned = is_pinned;
                }
                document.updated_at = Utc::now();
                validate_memory_document_scope(&document)?;

                diesel::delete(
                    agent_memory_chunks::table
                        .filter(agent_memory_chunks::document_id.eq(document_id.to_string())),
                )
                .execute(transaction)
                .map_err(|error| sqlite_error("clear memory chunks during update", error))?;

                let chunks = chunk_memory_document(&document);
                document.chunk_count = chunks.len();
                let updated = diesel::update(
                    agent_memory_documents::table
                        .filter(agent_memory_documents::id.eq(document.id.to_string())),
                )
                .set((
                    agent_memory_documents::namespace.eq(document.namespace.clone()),
                    agent_memory_documents::title.eq(document.title.clone()),
                    agent_memory_documents::updated_at.eq(document.updated_at.to_rfc3339()),
                    agent_memory_documents::data
                        .eq(serialize_record(&document, "memory document")?),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("update memory document", error))?;
                expect_changed(updated, "memory document")?;
                insert_memory_chunks_sqlite(transaction, &chunks)?;
                Ok(document)
            })?
        };

        if self.chroma.is_some() {
            self.reindex_memory_db().await?;
        }

        Ok(document)
    }

    pub(super) async fn delete_memory_document_db(&self, document_id: Uuid) -> CoreResult<()> {
        {
            let mut connection = self.open_connection()?;
            let deleted = diesel::delete(
                agent_memory_documents::table
                    .filter(agent_memory_documents::id.eq(document_id.to_string())),
            )
            .execute(&mut connection)
            .map_err(|error| sqlite_error("delete memory document", error))?;
            expect_changed(deleted, "memory document")?;
        }

        if self.chroma.is_some() {
            self.reindex_memory_db().await?;
        }

        Ok(())
    }

    pub(super) async fn get_memory_document_db(
        &self,
        document_id: Uuid,
    ) -> CoreResult<MemoryDocumentDetail> {
        let mut connection = self.open_connection()?;
        let document = load_json_record::<MemoryDocumentRecord, _>(
            &mut connection,
            agent_memory_documents::table
                .filter(agent_memory_documents::id.eq(document_id.to_string()))
                .select(agent_memory_documents::data),
            "memory document",
        )?;
        let chunks = load_json_records(
            &mut connection,
            agent_memory_chunks::table
                .filter(agent_memory_chunks::document_id.eq(document_id.to_string()))
                .order(agent_memory_chunks::ordinal.asc())
                .select(agent_memory_chunks::data),
            "memory chunk",
        )?;
        Ok(MemoryDocumentDetail { document, chunks })
    }

    pub(super) async fn search_memory_db(
        &self,
        payload: MemorySearchRequest,
    ) -> CoreResult<MemorySearchResult> {
        if payload.query.trim().is_empty() {
            return Err(CoreError::bad_request(
                "memory search query cannot be empty",
            ));
        }

        let hits = match self
            .search_memory_semantic(
                payload.query.trim(),
                payload.namespace.as_deref(),
                payload.memory_scopes.as_deref(),
                payload.owner_session_id,
                payload.limit.unwrap_or(5),
            )
            .await?
        {
            Some(hits) => hits,
            None => {
                let state = self.load_memory_state()?;
                search_memory_hits(
                    MemoryCorpus {
                        documents: &state.memory_documents,
                        chunks: &state.memory_chunks,
                    },
                    payload.query.trim(),
                    payload.namespace.as_deref(),
                    payload.memory_scopes.as_deref(),
                    payload.owner_session_id,
                    payload.limit.unwrap_or(5),
                )
            }
        };

        Ok(MemorySearchResult { hits })
    }

    pub(super) async fn reindex_memory_db(&self) -> CoreResult<ReindexResult> {
        let (documents, old_chunks) = {
            let mut connection = self.open_connection()?;
            let documents = load_json_records::<MemoryDocumentRecord, _>(
                &mut connection,
                agent_memory_documents::table
                    .order(agent_memory_documents::created_at.asc())
                    .select(agent_memory_documents::data),
                "memory document",
            )?;
            let old_chunks = load_json_records::<MemoryChunkRecord, _>(
                &mut connection,
                agent_memory_chunks::table
                    .order(agent_memory_chunks::ordinal.asc())
                    .select(agent_memory_chunks::data),
                "memory chunk",
            )?;
            (documents, old_chunks)
        };

        let (document_count, chunk_count, reindexed_documents) = {
            let mut connection = self.open_connection()?;
            connection.transaction::<_, CoreError, _>(|transaction| {
                diesel::delete(agent_memory_chunks::table)
                    .execute(transaction)
                    .map_err(|error| sqlite_error("clear memory chunks", error))?;

                let mut chunk_count = 0usize;
                let mut reindexed_documents = Vec::new();
                for mut document in documents.clone() {
                    let chunks = chunk_memory_document(&document);
                    document.chunk_count = chunks.len();
                    document.updated_at = Utc::now();
                    diesel::update(
                        agent_memory_documents::table
                            .filter(agent_memory_documents::id.eq(document.id.to_string())),
                    )
                    .set((
                        agent_memory_documents::updated_at.eq(document.updated_at.to_rfc3339()),
                        agent_memory_documents::data
                            .eq(serialize_record(&document, "memory document")?),
                    ))
                    .execute(transaction)
                    .map_err(|error| {
                        sqlite_error("update memory document during reindex", error)
                    })?;

                    insert_memory_chunks_sqlite(transaction, &chunks)?;
                    chunk_count += chunks.len();
                    reindexed_documents.push(document);
                }

                Ok((reindexed_documents.len(), chunk_count, reindexed_documents))
            })?
        };

        if let Some(chroma) = &self.chroma {
            let _old_ids = old_chunks
                .iter()
                .map(|chunk| chunk.id.to_string())
                .collect::<Vec<_>>();
            if let Err(error) = chroma.reset_collection().await {
                warn!(
                    "failed to reset chroma collection during reindex: {}",
                    error.message
                );
            }

            let records = reindexed_documents
                .into_iter()
                .flat_map(|document| chroma_records_for_document(&document))
                .collect::<Vec<_>>();
            if let Err(error) = chroma.upsert_records(records).await {
                warn!(
                    "failed to rebuild chroma index during reindex: {}",
                    error.message
                );
            }
        }

        Ok(ReindexResult {
            documents: document_count,
            chunks: chunk_count,
        })
    }
}

fn default_memory_scope_from_namespace(namespace: Option<&str>) -> MemoryScope {
    match namespace.unwrap_or("global") {
        "session" => MemoryScope::Session,
        "global" => MemoryScope::Global,
        _ => MemoryScope::Project,
    }
}

fn validate_memory_document_scope(document: &MemoryDocumentRecord) -> CoreResult<()> {
    if matches!(document.memory_scope, MemoryScope::Session) && document.owner_session_id.is_none()
    {
        return Err(CoreError::bad_request(
            "session-scoped memory requires ownerSessionId",
        ));
    }

    Ok(())
}
