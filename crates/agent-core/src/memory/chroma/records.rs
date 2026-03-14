use serde_json::json;

use crate::domain::MemoryDocumentRecord;

use super::{embeddings::embed_text, types::ChromaRecord};

pub(crate) fn chroma_records_for_document(document: &MemoryDocumentRecord) -> Vec<ChromaRecord> {
    crate::memory::chunk_memory_document(document)
        .into_iter()
        .map(|chunk| ChromaRecord {
            id: chunk.id.to_string(),
            embedding: embed_text(&chunk.content),
            document: chunk.content,
            metadata: json!({
                "document_id": document.id.to_string(),
                "document_title": document.title,
                "namespace": document.namespace,
                "ordinal": chunk.ordinal,
                "source": document.source
            }),
        })
        .collect()
}
