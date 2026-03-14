use uuid::Uuid;

use crate::domain::{MemoryChunkRecord, MemoryDocumentRecord};

use super::retrieval::extract_terms;

pub(crate) fn chunk_text(input: &str, chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for word in input.split_whitespace() {
        if !current.is_empty() && current.len() + word.len() + 1 > chunk_size {
            chunks.push(current.clone());
            current.clear();
        }

        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

pub(crate) fn chunk_memory_document(document: &MemoryDocumentRecord) -> Vec<MemoryChunkRecord> {
    chunk_text(&document.content, 180)
        .into_iter()
        .enumerate()
        .map(|(ordinal, content)| MemoryChunkRecord {
            id: Uuid::new_v5(
                &Uuid::NAMESPACE_OID,
                format!("{}:{ordinal}", document.id).as_bytes(),
            ),
            document_id: document.id,
            namespace: document.namespace.clone(),
            ordinal,
            keywords: extract_terms(&content),
            content,
        })
        .collect()
}
