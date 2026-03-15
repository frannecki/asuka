use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::domain::{MemoryChunkRecord, MemoryDocumentRecord, MemoryScope, MemorySearchHit};

pub(crate) struct MemoryCorpus<'a> {
    pub documents: &'a HashMap<Uuid, MemoryDocumentRecord>,
    pub chunks: &'a [MemoryChunkRecord],
}

pub(crate) fn search_memory_hits(
    corpus: MemoryCorpus<'_>,
    query: &str,
    namespace: Option<&str>,
    memory_scopes: Option<&[MemoryScope]>,
    owner_session_id: Option<Uuid>,
    limit: usize,
) -> Vec<MemorySearchHit> {
    let query_terms = extract_terms(query);
    if query_terms.is_empty() {
        return Vec::new();
    }

    let hits = corpus
        .chunks
        .iter()
        .filter(|chunk| {
            namespace
                .map(|value| chunk.namespace == value)
                .unwrap_or(true)
        })
        .filter_map(|chunk| {
            let document = corpus.documents.get(&chunk.document_id)?;
            if !matches_memory_filters(document, memory_scopes, owner_session_id) {
                return None;
            }

            let overlap = query_terms
                .iter()
                .filter(|term| chunk.keywords.contains(term))
                .count();

            if overlap == 0 {
                return None;
            }

            Some(MemorySearchHit {
                document_id: document.id,
                chunk_id: chunk.id,
                document_title: document.title.clone(),
                namespace: chunk.namespace.clone(),
                memory_scope: document.memory_scope.clone(),
                owner_session_id: document.owner_session_id,
                content: chunk.content.clone(),
                score: overlap as f32 / query_terms.len() as f32,
            })
        })
        .collect::<Vec<_>>();

    merge_memory_hits([hits], limit)
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

pub(crate) fn merge_memory_hits<I>(hit_sets: I, limit: usize) -> Vec<MemorySearchHit>
where
    I: IntoIterator<Item = Vec<MemorySearchHit>>,
{
    let mut seen = HashSet::new();
    let mut hits = hit_sets
        .into_iter()
        .flatten()
        .filter(|hit| seen.insert(hit.chunk_id))
        .collect::<Vec<_>>();

    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                memory_scope_rank(&left.memory_scope).cmp(&memory_scope_rank(&right.memory_scope))
            })
    });
    hits.truncate(limit.max(1));
    hits
}

fn matches_memory_filters(
    document: &MemoryDocumentRecord,
    memory_scopes: Option<&[MemoryScope]>,
    owner_session_id: Option<Uuid>,
) -> bool {
    let scope_matches = memory_scopes
        .map(|scopes| scopes.iter().any(|scope| *scope == document.memory_scope))
        .unwrap_or(true);
    if !scope_matches {
        return false;
    }

    owner_session_id
        .map(|session_id| document.owner_session_id == Some(session_id))
        .unwrap_or(true)
}

fn memory_scope_rank(scope: &MemoryScope) -> u8 {
    match scope {
        MemoryScope::Session => 0,
        MemoryScope::Project => 1,
        MemoryScope::Global => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use uuid::Uuid;

    use super::{extract_terms, search_memory_hits, MemoryCorpus};
    use crate::domain::{MemoryChunkRecord, MemoryDocumentRecord, MemoryScope};

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
                    memory_scope: MemoryScope::Project,
                    owner_session_id: None,
                    owner_task_id: None,
                    is_pinned: false,
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
                    memory_scope: MemoryScope::Global,
                    owner_session_id: None,
                    owner_task_id: None,
                    is_pinned: false,
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
            Some(&[MemoryScope::Project]),
            None,
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
            None,
            None,
            3,
        );

        assert!(hits.is_empty());
    }

    #[test]
    fn search_memory_hits_can_filter_session_scoped_documents() {
        let session_id = Uuid::new_v4();
        let session_doc_id = Uuid::new_v4();
        let global_doc_id = Uuid::new_v4();

        let documents = HashMap::from([
            (
                session_doc_id,
                MemoryDocumentRecord {
                    id: session_doc_id,
                    title: "Session note".to_string(),
                    namespace: "session".to_string(),
                    source: "test".to_string(),
                    memory_scope: MemoryScope::Session,
                    owner_session_id: Some(session_id),
                    owner_task_id: None,
                    is_pinned: false,
                    content: "session-specific rust note".to_string(),
                    summary: String::new(),
                    chunk_count: 1,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
            ),
            (
                global_doc_id,
                MemoryDocumentRecord {
                    id: global_doc_id,
                    title: "Global note".to_string(),
                    namespace: "global".to_string(),
                    source: "test".to_string(),
                    memory_scope: MemoryScope::Global,
                    owner_session_id: None,
                    owner_task_id: None,
                    is_pinned: false,
                    content: "global rust note".to_string(),
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
                document_id: session_doc_id,
                namespace: "session".to_string(),
                ordinal: 0,
                content: "session-specific rust note".to_string(),
                keywords: vec!["session".into(), "rust".into(), "note".into()],
            },
            MemoryChunkRecord {
                id: Uuid::new_v4(),
                document_id: global_doc_id,
                namespace: "global".to_string(),
                ordinal: 0,
                content: "global rust note".to_string(),
                keywords: vec!["global".into(), "rust".into(), "note".into()],
            },
        ];

        let hits = search_memory_hits(
            MemoryCorpus {
                documents: &documents,
                chunks: &chunks,
            },
            "session rust note",
            None,
            Some(&[MemoryScope::Session]),
            Some(session_id),
            5,
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_title, "Session note");
        assert_eq!(hits[0].memory_scope, MemoryScope::Session);
        assert_eq!(hits[0].owner_session_id, Some(session_id));
    }
}
