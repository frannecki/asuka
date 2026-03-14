use chrono::Utc;
use rusqlite::params;
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
        insert_memory_chunks_sqlite, insert_memory_document_and_chunks_sqlite, query_json_records,
        serialize_record, sqlite_error,
    },
    store::SqliteStore,
};

impl SqliteStore {
    pub(super) async fn write_run_memory_note_db(
        &self,
        user_content: &str,
        response: &str,
    ) -> CoreResult<MemoryDocumentRecord> {
        let document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: format!("Run note {}", Utc::now().format("%Y-%m-%d %H:%M:%S")),
            namespace: "session".to_string(),
            source: "run-summary".to_string(),
            content: format!("User request: {user_content}\n\nAssistant response: {response}"),
            summary: summarize_text(response, 24),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let document = {
            let mut connection = self.open_connection()?;
            let transaction = connection
                .transaction()
                .map_err(|error| sqlite_error("begin write run memory note transaction", error))?;
            let document = insert_memory_document_and_chunks_sqlite(&transaction, &document)?;
            transaction
                .commit()
                .map_err(|error| sqlite_error("commit write run memory note transaction", error))?;
            document
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
        let connection = self.open_connection()?;
        query_json_records(
            &connection,
            "SELECT data FROM agent_memory_documents ORDER BY updated_at DESC",
            [],
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

        let document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: payload.title.trim().to_string(),
            namespace: payload
                .namespace
                .unwrap_or_else(|| "global".to_string())
                .trim()
                .to_string(),
            source: payload
                .source
                .unwrap_or_else(|| "manual".to_string())
                .trim()
                .to_string(),
            content: payload.content.trim().to_string(),
            summary: summarize_text(payload.content.trim(), 20),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let document = {
            let mut connection = self.open_connection()?;
            let transaction = connection
                .transaction()
                .map_err(|error| sqlite_error("begin create memory document transaction", error))?;
            let document = insert_memory_document_and_chunks_sqlite(&transaction, &document)?;
            transaction.commit().map_err(|error| {
                sqlite_error("commit create memory document transaction", error)
            })?;
            document
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

    pub(super) async fn get_memory_document_db(
        &self,
        document_id: Uuid,
    ) -> CoreResult<MemoryDocumentDetail> {
        let connection = self.open_connection()?;
        let document = super::helpers::get_json_record_by_id::<MemoryDocumentRecord>(
            &connection,
            "agent_memory_documents",
            document_id,
            "memory document",
        )?;
        let chunks = query_json_records(
            &connection,
            "SELECT data FROM agent_memory_chunks WHERE document_id = ?1 ORDER BY ordinal ASC",
            [document_id.to_string()],
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
                    payload.limit.unwrap_or(5),
                )
            }
        };

        Ok(MemorySearchResult { hits })
    }

    pub(super) async fn reindex_memory_db(&self) -> CoreResult<ReindexResult> {
        let connection = self.open_connection()?;
        let documents = query_json_records::<MemoryDocumentRecord, _>(
            &connection,
            "SELECT data FROM agent_memory_documents ORDER BY created_at ASC",
            [],
            "memory document",
        )?;
        let old_chunks = query_json_records::<MemoryChunkRecord, _>(
            &connection,
            "SELECT data FROM agent_memory_chunks ORDER BY ordinal ASC",
            [],
            "memory chunk",
        )?;

        let (document_count, chunk_count, reindexed_documents) = {
            let mut connection = self.open_connection()?;
            let transaction = connection
                .transaction()
                .map_err(|error| sqlite_error("begin reindex memory transaction", error))?;
            transaction
                .execute("DELETE FROM agent_memory_chunks", [])
                .map_err(|error| sqlite_error("clear memory chunks", error))?;

            let mut chunk_count = 0usize;
            let mut reindexed_documents = Vec::new();
            for mut document in documents {
                let chunks = chunk_memory_document(&document);
                document.chunk_count = chunks.len();
                document.updated_at = Utc::now();
                let data = serialize_record(&document, "memory document")?;
                transaction
                    .execute(
                        r#"
                        UPDATE agent_memory_documents
                        SET updated_at = ?2, data = ?3
                        WHERE id = ?1
                        "#,
                        params![
                            document.id.to_string(),
                            document.updated_at.to_rfc3339(),
                            data
                        ],
                    )
                    .map_err(|error| {
                        sqlite_error("update memory document during reindex", error)
                    })?;

                insert_memory_chunks_sqlite(&transaction, &chunks)?;
                chunk_count += chunks.len();
                reindexed_documents.push(document);
            }

            transaction
                .commit()
                .map_err(|error| sqlite_error("commit reindex memory transaction", error))?;
            (reindexed_documents.len(), chunk_count, reindexed_documents)
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
