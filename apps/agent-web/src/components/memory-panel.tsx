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

  useEffect(() => {
    let cancelled = false;

    void listMemoryDocuments().then((nextDocuments) => {
      if (!cancelled) {
        setDocuments(nextDocuments);
      }
    });

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
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
  }

  async function handleSearch(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const result = await searchMemory({
      query,
      namespace,
      memoryScopes: searchScope === "all" ? undefined : [searchScope],
      ownerSessionId: searchOwnerSessionId || null,
      limit: 6,
    });
    setHits(result.hits);
  }

  async function handleSelect(documentId: string) {
    const detail = await getMemoryDocument(documentId);
    setSelected(detail);
  }

  async function handleReindex() {
    const result = await reindexMemory();
    setFeedback(`Reindexed ${result.documents} documents into ${result.chunks} chunks.`);
  }

  return (
    <div className="memory-layout">
      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Memory</p>
            <h2>Long-term document store</h2>
          </div>
          <button className="ghost-button" onClick={() => void handleReindex()} type="button">
            Reindex
          </button>
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
              rows={6}
              value={content}
            />
          </label>
          <button className="primary-button" type="submit">
            Save memory
          </button>
        </form>

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
              onChange={(event) =>
                setSearchScope(event.target.value as "all" | MemoryScope)
              }
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

        {feedback ? <p className="hint-copy">{feedback}</p> : null}

        {hits.length > 0 ? (
          <div className="stack-gap">
            {hits.map((hit) => (
              <article className="activity-card" key={hit.chunkId}>
                <div className="activity-topline">
                  <strong>{hit.documentTitle}</strong>
                  <span>{hit.score.toFixed(2)}</span>
                </div>
                <p className="hint-copy">
                  {hit.memoryScope} · {hit.namespace}
                  {hit.ownerSessionId ? ` · ${hit.ownerSessionId}` : ""}
                </p>
                <p>{hit.content}</p>
              </article>
            ))}
          </div>
        ) : null}
      </section>

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Documents</p>
            <h2>Indexed memory</h2>
          </div>
        </div>

        <div className="session-list">
          {documents.map((document) => (
            <button
              className="session-card"
              key={document.id}
              onClick={() => void handleSelect(document.id)}
              type="button"
            >
              <strong>{document.title}</strong>
              <span>
                {document.memoryScope} · {document.namespace} · {document.chunkCount} chunks
              </span>
              <span>{document.summary}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Detail</p>
            <h2>Chunk view</h2>
          </div>
        </div>

        {selected ? (
          <div className="stack-gap">
            <p className="hint-copy">
              {selected.document.memoryScope} · {selected.document.namespace} ·{" "}
              {selected.document.source}
            </p>
            {selected.document.ownerSessionId ? (
              <p className="hint-copy">owner session: {selected.document.ownerSessionId}</p>
            ) : null}
            <p>{selected.document.content}</p>
            <div className="stack-gap">
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
          <div className="empty-state">Select a memory document to inspect its chunks.</div>
        )}
      </section>
    </div>
  );
}
