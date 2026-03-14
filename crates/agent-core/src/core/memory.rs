use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{
        CreateMemoryDocumentRequest, MemoryDocumentDetail, MemoryDocumentRecord,
        MemorySearchRequest, MemorySearchResult, ReindexResult,
    },
    error::CoreResult,
};

impl AgentCore {
    pub async fn list_memory_documents(&self) -> CoreResult<Vec<MemoryDocumentRecord>> {
        self.store.list_memory_documents().await
    }

    pub async fn create_memory_document(
        &self,
        payload: CreateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        self.store.create_memory_document(payload).await
    }

    pub async fn get_memory_document(&self, document_id: Uuid) -> CoreResult<MemoryDocumentDetail> {
        self.store.get_memory_document(document_id).await
    }

    pub async fn search_memory(
        &self,
        payload: MemorySearchRequest,
    ) -> CoreResult<MemorySearchResult> {
        self.store.search_memory(payload).await
    }

    pub async fn reindex_memory(&self) -> CoreResult<ReindexResult> {
        self.store.reindex_memory().await
    }
}
