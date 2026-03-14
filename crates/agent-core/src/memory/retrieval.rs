use std::collections::HashMap;

use uuid::Uuid;

use crate::domain::{MemoryChunkRecord, MemoryDocumentRecord, MemorySearchHit};

pub(crate) struct MemoryCorpus<'a> {
    pub documents: &'a HashMap<Uuid, MemoryDocumentRecord>,
    pub chunks: &'a [MemoryChunkRecord],
}

pub(crate) fn search_memory_hits(
    corpus: MemoryCorpus<'_>,
    query: &str,
    namespace: Option<&str>,
    limit: usize,
) -> Vec<MemorySearchHit> {
    let query_terms = extract_terms(query);
    if query_terms.is_empty() {
        return Vec::new();
    }

    let mut hits = corpus
        .chunks
        .iter()
        .filter(|chunk| {
            namespace
                .map(|value| chunk.namespace == value)
                .unwrap_or(true)
        })
        .filter_map(|chunk| {
            let overlap = query_terms
                .iter()
                .filter(|term| chunk.keywords.contains(term))
                .count();

            if overlap == 0 {
                return None;
            }

            let document = corpus.documents.get(&chunk.document_id)?;
            Some(MemorySearchHit {
                document_id: document.id,
                chunk_id: chunk.id,
                document_title: document.title.clone(),
                namespace: chunk.namespace.clone(),
                content: chunk.content.clone(),
                score: overlap as f32 / query_terms.len() as f32,
            })
        })
        .collect::<Vec<_>>();

    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit.max(1));
    hits
}

pub(crate) fn extract_terms(input: &str) -> Vec<String> {
    let mut terms = input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.len() > 2)
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use uuid::Uuid;

    use super::{extract_terms, search_memory_hits, MemoryCorpus};
    use crate::domain::{MemoryChunkRecord, MemoryDocumentRecord};

    #[test]
    fn extract_terms_normalizes_and_deduplicates() {
        let terms = extract_terms("Rust rust, agent-core! API 42 API");
        assert_eq!(
            terms,
            vec![
                "agent".to_string(),
                "api".to_string(),
                "core".to_string(),
                "rust".to_string()
            ]
        );
    }

    #[test]
    fn search_memory_hits_ranks_by_overlap_and_filters_namespace() {
        let document_id = Uuid::new_v4();
        let other_document_id = Uuid::new_v4();

        let documents = HashMap::from([
            (
                document_id,
                MemoryDocumentRecord {
                    id: document_id,
                    title: "Primary".to_string(),
                    namespace: "project".to_string(),
                    source: "test".to_string(),
                    content: "rust agent memory".to_string(),
                    summary: String::new(),
                    chunk_count: 1,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
            ),
            (
                other_document_id,
                MemoryDocumentRecord {
                    id: other_document_id,
                    title: "Other".to_string(),
                    namespace: "global".to_string(),
                    source: "test".to_string(),
                    content: "rust platform".to_string(),
                    summary: String::new(),
                    chunk_count: 1,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
            ),
        ]);

        let chunks = vec![
            MemoryChunkRecord {
                id: Uuid::new_v4(),
                document_id,
                namespace: "project".to_string(),
                ordinal: 0,
                content: "rust agent memory".to_string(),
                keywords: vec!["rust".into(), "agent".into(), "memory".into()],
            },
            MemoryChunkRecord {
                id: Uuid::new_v4(),
                document_id: other_document_id,
                namespace: "global".to_string(),
                ordinal: 0,
                content: "rust platform".to_string(),
                keywords: vec!["rust".into(), "platform".into()],
            },
        ];

        let hits = search_memory_hits(
            MemoryCorpus {
                documents: &documents,
                chunks: &chunks,
            },
            "rust agent memory",
            Some("project"),
            5,
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_title, "Primary");
        assert_eq!(hits[0].namespace, "project");
        assert!(hits[0].score > 0.9);
    }

    #[test]
    fn search_memory_hits_returns_empty_for_empty_query_terms() {
        let documents = HashMap::new();
        let chunks = Vec::new();

        let hits = search_memory_hits(
            MemoryCorpus {
                documents: &documents,
                chunks: &chunks,
            },
            "a b !!",
            None,
            3,
        );

        assert!(hits.is_empty());
    }
}
