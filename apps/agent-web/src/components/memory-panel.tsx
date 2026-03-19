"use client";

import { useEffect, useState } from "react";

import {
  createMemoryDocument,
  getMemoryDocument,
  listMemoryDocuments,
  reindexMemory,
  searchMemory,
} from "@/lib/api";
import type {
  MemoryDocumentDetail,
  MemoryDocumentRecord,
  MemoryScope,
  MemorySearchHit,
} from "@/lib/types";
import { excerpt, humanizeLabel } from "@/lib/view";

export function MemoryPanel() {
  const [documents, setDocuments] = useState<MemoryDocumentRecord[]>([]);
  const [title, setTitle] = useState("");
  const [namespace, setNamespace] = useState("knowledge");
  const [memoryScope, setMemoryScope] = useState<MemoryScope>("project");
  const [ownerSessionId, setOwnerSessionId] = useState("");
  const [content, setContent] = useState("");
  const [query, setQuery] = useState("");
  const [searchScope, setSearchScope] = useState<"all" | MemoryScope>("all");
  const [searchOwnerSessionId, setSearchOwnerSessionId] = useState("");
  const [hits, setHits] = useState<MemorySearchHit[]>([]);
  const [selected, setSelected] = useState<MemoryDocumentDetail | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void listMemoryDocuments()
      .then((nextDocuments) => {
        if (!cancelled) {
          setDocuments(nextDocuments);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(
            loadError instanceof Error
              ? loadError.message
              : "Failed to load memory documents.",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    try {
      const created = await createMemoryDocument({
        title,
        namespace,
        memoryScope,
        ownerSessionId: ownerSessionId || null,
        content,
        source: "manual",
      });
      setDocuments((current) => [created, ...current]);
      setTitle("");
      setContent("");
      setOwnerSessionId("");
      setFeedback(`Created memory document ${created.title}.`);
      setError(null);
    } catch (createError) {
      setError(
        createError instanceof Error
          ? createError.message
          : "Failed to create memory document.",
      );
    }
  }

  async function handleSearch(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    try {
      const result = await searchMemory({
        query,
        namespace,
        memoryScopes: searchScope === "all" ? undefined : [searchScope],
        ownerSessionId: searchOwnerSessionId || null,
        limit: 6,
      });
      setHits(result.hits);
      setError(null);
    } catch (searchError) {
      setError(
        searchError instanceof Error
          ? searchError.message
          : "Failed to search memory.",
      );
    }
  }

  async function handleSelect(documentId: string) {
    try {
      const detail = await getMemoryDocument(documentId);
      setSelected(detail);
      setError(null);
    } catch (selectError) {
      setError(
        selectError instanceof Error
          ? selectError.message
          : "Failed to load memory detail.",
      );
    }
  }

  async function handleReindex() {
    try {
      const result = await reindexMemory();
      setFeedback(`Reindexed ${result.documents} documents into ${result.chunks} chunks.`);
      setError(null);
    } catch (reindexError) {
      setError(
        reindexError instanceof Error
          ? reindexError.message
          : "Failed to reindex memory.",
      );
    }
  }

  return (
    <div className="memory-layout">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Global memory</p>
            <h2>Operate the retrieval corpus, not just the conversation.</h2>
          </div>
          <p>
            Create durable documents, inspect chunked detail, and test the
            retrieval path that powers session, project, and global recall.
          </p>
          <div className="hero-actions">
            <button className="ghost-button" onClick={() => void handleReindex()} type="button">
              Reindex
            </button>
          </div>
          {feedback ? <p className="status-pill tone-mint">{feedback}</p> : null}
          {error ? <p className="error-copy">{error}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{documents.length}</strong>
              <span>documents</span>
            </article>
            <article className="hero-stat">
              <strong>{hits.length}</strong>
              <span>search hits</span>
            </article>
          </div>
        </div>
      </section>

      <section className="story-grid">
        <section className="panel span-6 stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Create</p>
              <h2>Add a memory document</h2>
            </div>
          </div>
          <form className="form-grid" onSubmit={handleCreate}>
            <label>
              Title
              <input
                className="text-input"
                onChange={(event) => setTitle(event.target.value)}
                value={title}
              />
            </label>
            <label>
              Namespace
              <input
                className="text-input"
                onChange={(event) => setNamespace(event.target.value)}
                value={namespace}
              />
            </label>
            <label>
              Scope
              <select
                className="text-input"
                onChange={(event) => setMemoryScope(event.target.value as MemoryScope)}
                value={memoryScope}
              >
                <option value="project">project</option>
                <option value="global">global</option>
                <option value="session">session</option>
              </select>
            </label>
            <label>
              Owner session ID
              <input
                className="text-input"
                onChange={(event) => setOwnerSessionId(event.target.value)}
                placeholder="Required for session scope"
                value={ownerSessionId}
              />
            </label>
            <label>
              Content
              <textarea
                className="text-input"
                onChange={(event) => setContent(event.target.value)}
                rows={7}
                value={content}
              />
            </label>
            <button className="primary-button" type="submit">
              Save memory
            </button>
          </form>
        </section>

        <section className="panel span-6 stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Search</p>
              <h2>Probe the retrieval path</h2>
            </div>
          </div>
          <form className="form-grid" onSubmit={handleSearch}>
            <label>
              Search query
              <input
                className="text-input"
                onChange={(event) => setQuery(event.target.value)}
                value={query}
              />
            </label>
            <label>
              Search scope
              <select
                className="text-input"
                onChange={(event) => setSearchScope(event.target.value as "all" | MemoryScope)}
                value={searchScope}
              >
                <option value="all">all</option>
                <option value="project">project</option>
                <option value="global">global</option>
                <option value="session">session</option>
              </select>
            </label>
            <label>
              Search owner session ID
              <input
                className="text-input"
                onChange={(event) => setSearchOwnerSessionId(event.target.value)}
                placeholder="Optional session filter"
                value={searchOwnerSessionId}
              />
            </label>
            <button className="ghost-button" type="submit">
              Search memory
            </button>
          </form>

          {hits.length > 0 ? (
            <div className="activity-list">
              {hits.map((hit) => (
                <article className="activity-card" key={hit.chunkId}>
                  <div className="activity-topline">
                    <strong>{hit.documentTitle}</strong>
                    <span>{hit.score.toFixed(2)}</span>
                  </div>
                  <p className="hint-copy">
                    {humanizeLabel(hit.memoryScope)} · {hit.namespace}
                    {hit.ownerSessionId ? ` · ${hit.ownerSessionId}` : ""}
                  </p>
                  <p>{excerpt(hit.content, 180)}</p>
                </article>
              ))}
            </div>
          ) : (
            <div className="empty-state small">
              Search results will appear here after you query the corpus.
            </div>
          )}
        </section>
      </section>

      <section className="story-grid">
        <section className="panel span-6 stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Documents</p>
              <h2>Indexed memory</h2>
            </div>
          </div>

          <div className="catalog-stack">
            {documents.map((document) => (
              <button
                className="catalog-card"
                key={document.id}
                onClick={() => void handleSelect(document.id)}
                type="button"
              >
                <strong>{document.title}</strong>
                <p>
                  {humanizeLabel(document.memoryScope)} · {document.namespace} ·{" "}
                  {document.chunkCount} chunks
                </p>
                <p>{excerpt(document.summary, 120)}</p>
              </button>
            ))}
          </div>
        </section>

        <section className="panel span-6 stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Detail</p>
              <h2>Chunk view</h2>
            </div>
          </div>

          {selected ? (
            <div className="stack-gap">
              <div className="status-strip">
                <span className="status-pill">{humanizeLabel(selected.document.memoryScope)}</span>
                <span className="status-pill">{selected.document.namespace}</span>
                {selected.document.isPinned ? <span className="status-pill tone-sun">Pinned</span> : null}
              </div>
              <article className="story-card">
                <strong>{selected.document.title}</strong>
                <p>{selected.document.content}</p>
              </article>
              <div className="activity-list">
                {selected.chunks.map((chunk) => (
                  <article className="activity-card" key={chunk.id}>
                    <div className="activity-topline">
                      <strong>Chunk {chunk.ordinal + 1}</strong>
                      <span>{chunk.keywords.length} keywords</span>
                    </div>
                    <p>{chunk.content}</p>
                  </article>
                ))}
              </div>
            </div>
          ) : (
            <div className="empty-state small">
              Select a document to inspect its full content and chunk layout.
            </div>
          )}
        </section>
      </section>
    </div>
  );
}
