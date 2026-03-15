use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{
        CreateMemoryDocumentRequest, MemoryDocumentDetail, MemoryDocumentRecord, MemorySearchHit,
        MemorySearchRequest, MemorySearchResult, ReindexResult, SessionMemoryOverview,
        SessionMemoryRetrievalRecord, UpdateMemoryDocumentRequest,
    },
    error::CoreResult,
    memory::summarize_text,
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

    pub async fn update_memory_document(
        &self,
        document_id: Uuid,
        payload: UpdateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        self.store
            .update_memory_document(document_id, payload)
            .await
    }

    pub async fn delete_memory_document(&self, document_id: Uuid) -> CoreResult<()> {
        self.store.delete_memory_document(document_id).await
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

    pub async fn get_session_memory_overview(
        &self,
        session_id: Uuid,
    ) -> CoreResult<SessionMemoryOverview> {
        let session = self.get_session(session_id).await?;
        let documents = self.store.list_memory_documents().await?;
        let short_term_summary = summarize_text(
            &session
                .messages
                .iter()
                .rev()
                .take(6)
                .map(|message| format!("{:?}: {}", message.role, message.content))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n"),
            40,
        );
        let mut scoped_documents = documents
            .into_iter()
            .filter(|document| document.owner_session_id == Some(session_id))
            .collect::<Vec<_>>();
        scoped_documents.sort_by_key(|document| std::cmp::Reverse(document.updated_at));
        let pinned_documents = scoped_documents
            .iter()
            .filter(|document| document.is_pinned)
            .cloned()
            .collect::<Vec<_>>();
        let tasks = self.list_tasks(Some(session_id)).await?;
        let mut recent_retrievals = Vec::new();
        for task in tasks.iter().take(6) {
            let runs = self.store.list_task_runs(task.id).await?;
            for run in runs.into_iter().take(3) {
                let history = self.list_run_events(run.id, None).await?;
                if let Some(retrieval) = history
                    .events
                    .into_iter()
                    .find(|event| event.event_type == "memory.retrieved")
                    .and_then(|event| {
                        parse_memory_retrieval(&event.payload).map(|hits| {
                            SessionMemoryRetrievalRecord {
                                run_id: run.id,
                                task_id: task.id,
                                timestamp: event.timestamp,
                                hits,
                            }
                        })
                    })
                {
                    recent_retrievals.push(retrieval);
                }
            }
        }
        recent_retrievals.sort_by_key(|retrieval| std::cmp::Reverse(retrieval.timestamp));
        recent_retrievals.truncate(6);

        Ok(SessionMemoryOverview {
            session_id,
            short_term_summary,
            scoped_documents,
            pinned_documents,
            recent_retrievals,
        })
    }

    pub async fn summarize_session_memory(
        &self,
        session_id: Uuid,
    ) -> CoreResult<MemoryDocumentRecord> {
        let session = self.get_session(session_id).await?;
        let transcript = session
            .messages
            .iter()
            .rev()
            .take(10)
            .map(|message| format!("{:?}: {}", message.role, message.content))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        self.create_memory_document(CreateMemoryDocumentRequest {
            title: format!(
                "Session summary {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
            ),
            namespace: Some("session".to_string()),
            source: Some("session-summary".to_string()),
            memory_scope: Some(crate::domain::MemoryScope::Session),
            owner_session_id: Some(session_id),
            owner_task_id: None,
            is_pinned: Some(true),
            content: format!(
                "Session short-term summary:\n{}\n\nRecent transcript:\n{}",
                summarize_text(&transcript, 60),
                transcript
            ),
        })
        .await
    }
}

fn parse_memory_retrieval(payload: &serde_json::Value) -> Option<Vec<MemorySearchHit>> {
    serde_json::from_value(payload.get("hits")?.clone()).ok()
}
