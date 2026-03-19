"use client";

import { useCallback, useEffect, useState } from "react";

import {
  deleteMemoryDocument,
  getMemoryDocument,
  getSessionMemoryOverview,
  summarizeSessionMemory,
  updateMemoryDocument,
} from "@/lib/api";
import type {
  MemoryDocumentDetail,
  MemoryDocumentRecord,
  MemoryScope,
  SessionMemoryOverview,
} from "@/lib/types";
import { compactId, excerpt, formatDateTime, humanizeLabel } from "@/lib/view";

type SessionMemoryViewProps = {
  sessionId: string;
};

export function SessionMemoryView({ sessionId }: SessionMemoryViewProps) {
  const [overview, setOverview] = useState<SessionMemoryOverview | null>(null);
  const [selected, setSelected] = useState<MemoryDocumentDetail | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);

  const loadOverview = useCallback(async () => {
    try {
      const nextOverview = await getSessionMemoryOverview(sessionId);
      setOverview(nextOverview);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Failed to load session memory.",
      );
    }
  }, [sessionId]);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  async function handleSelect(documentId: string) {
    try {
      const detail = await getMemoryDocument(documentId);
      setSelected(detail);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Failed to load memory detail.",
      );
    }
  }

  async function handleSummarize() {
    setBusyKey("summarize");
    try {
      const document = await summarizeSessionMemory(sessionId);
      await loadOverview();
      await handleSelect(document.id);
      setFeedback(`Saved a pinned session summary as ${document.title}.`);
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to summarize session memory.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handleTogglePin(document: MemoryDocumentRecord) {
    setBusyKey(`pin:${document.id}`);
    try {
      const updated = await updateMemoryDocument(document.id, {
        isPinned: !document.isPinned,
        memoryScope: document.memoryScope,
        namespace: document.namespace,
        ownerSessionId: document.ownerSessionId,
        title: document.title,
      });
      await loadOverview();
      if (selected?.document.id === document.id) {
        await handleSelect(updated.id);
      }
      setFeedback(
        updated.isPinned
          ? `Pinned ${updated.title} in this session.`
          : `Unpinned ${updated.title}.`,
      );
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to update memory pin state.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handlePromote(document: MemoryDocumentRecord, memoryScope: MemoryScope) {
    setBusyKey(`scope:${document.id}:${memoryScope}`);
    try {
      const updated = await updateMemoryDocument(document.id, {
        title: document.title,
        namespace: document.namespace,
        memoryScope,
        ownerSessionId: memoryScope === "session" ? sessionId : null,
        isPinned: document.isPinned,
      });
      await loadOverview();
      if (selected?.document.id === document.id) {
        await handleSelect(updated.id);
      }
      setFeedback(`Moved ${updated.title} to ${memoryScope} scope.`);
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to update memory scope.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handleForget(document: MemoryDocumentRecord) {
    setBusyKey(`forget:${document.id}`);
    try {
      await deleteMemoryDocument(document.id);
      if (selected?.document.id === document.id) {
        setSelected(null);
      }
      await loadOverview();
      setFeedback(`Forgot ${document.title} from this session.`);
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to forget the memory document.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  const scopedDocuments = overview?.scopedDocuments ?? [];
  const pinnedDocuments = overview?.pinnedDocuments ?? [];

  return (
    <div className="stack-gap">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Session memory</p>
            <h2>Short-term recall, pinned notes, and retrieval traces.</h2>
          </div>
          <p>
            This view reads the session memory overview, lets you summarize the
            conversation back into durable memory, and promotes useful notes out
            to project or global scope.
          </p>
          <div className="hero-actions">
            <button
              className="primary-button"
              disabled={busyKey !== null}
              onClick={() => void handleSummarize()}
              type="button"
            >
              Summarize session
            </button>
          </div>
          {error ? <p className="error-copy">{error}</p> : null}
          {feedback ? <p className="status-pill tone-mint">{feedback}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{scopedDocuments.length}</strong>
              <span>scoped docs</span>
            </article>
            <article className="hero-stat">
              <strong>{pinnedDocuments.length}</strong>
              <span>pinned docs</span>
            </article>
          </div>
          <article className="hero-orb">
            <p className="eyebrow">Short-term summary</p>
            <strong>{overview?.shortTermSummary || "No short-term summary yet."}</strong>
          </article>
        </div>
      </section>

      <section className="memory-overview-grid">
        <article className="memory-summary-card">
          <p className="eyebrow">Pinned notes</p>
          <div className="session-chip-list">
            {pinnedDocuments.map((document) => (
              <button
                className="timeline-chip artifact"
                key={document.id}
                onClick={() => void handleSelect(document.id)}
                type="button"
              >
                {document.title}
              </button>
            ))}
            {pinnedDocuments.length === 0 ? (
              <span className="story-kicker">
                Pin the most useful session memories to keep them visible.
              </span>
            ) : null}
          </div>
        </article>
      </section>

      <section className="session-memory-grid">
        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Scoped memory</p>
              <h2>Session documents</h2>
            </div>
          </div>

          <div className="session-memory-list">
            {scopedDocuments.map((document) => (
              <article
                className={`session-memory-card${
                  selected?.document.id === document.id ? " is-active" : ""
                }`}
                key={document.id}
              >
                <button
                  className="session-memory-select"
                  onClick={() => void handleSelect(document.id)}
                  type="button"
                >
                  <div className="activity-copy">
                    <strong>{document.title}</strong>
                    <p>{excerpt(document.summary, 120)}</p>
                  </div>
                  <div className="status-strip">
                    <span className="status-pill">{humanizeLabel(document.memoryScope)}</span>
                    {document.isPinned ? <span className="status-pill tone-sun">Pinned</span> : null}
                    <span className="status-pill">{document.namespace}</span>
                  </div>
                </button>
                <div className="session-memory-actions">
                  <button
                    className="ghost-button"
                    disabled={busyKey !== null}
                    onClick={() => void handleTogglePin(document)}
                    type="button"
                  >
                    {document.isPinned ? "Unpin" : "Pin"}
                  </button>
                  <button
                    className="ghost-button"
                    disabled={busyKey !== null}
                    onClick={() => void handlePromote(document, "project")}
                    type="button"
                  >
                    Promote to project
                  </button>
                  <button
                    className="ghost-button"
                    disabled={busyKey !== null}
                    onClick={() => void handlePromote(document, "global")}
                    type="button"
                  >
                    Promote to global
                  </button>
                  <button
                    className="ghost-button danger-button"
                    disabled={busyKey !== null}
                    onClick={() => void handleForget(document)}
                    type="button"
                  >
                    Forget
                  </button>
                </div>
              </article>
            ))}
            {scopedDocuments.length === 0 ? (
              <div className="empty-state small">
                No session-scoped memory documents yet. Ask the agent to remember
                something or summarize the session to seed local memory.
              </div>
            ) : null}
          </div>
        </article>

        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Recent retrievals</p>
              <h2>Memory hits used in runs</h2>
            </div>
          </div>

          <div className="session-memory-retrievals">
            {overview?.recentRetrievals.map((retrieval) => (
              <article className="activity-card" key={`${retrieval.runId}-${retrieval.timestamp}`}>
                <div className="activity-topline">
                  <span className="activity-badge">Run {compactId(retrieval.runId)}</span>
                  <span>{formatDateTime(retrieval.timestamp)}</span>
                </div>
                <div className="activity-copy">
                  <strong>{retrieval.hits.length} retrieved hit(s)</strong>
                  <p>Task {compactId(retrieval.taskId)}</p>
                </div>
                <div className="session-chip-list">
                  {retrieval.hits.slice(0, 4).map((hit) => (
                    <span className="timeline-chip" key={hit.chunkId}>
                      {hit.documentTitle}
                    </span>
                  ))}
                </div>
              </article>
            ))}
            {overview?.recentRetrievals.length ? null : (
              <div className="empty-state small">
                Recent retrievals appear here after the agent pulls session or
                long-term memory into a run.
              </div>
            )}
          </div>
        </article>

        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Selected memory</p>
              <h2>Document detail</h2>
            </div>
          </div>

          {selected ? (
            <div className="stack-gap">
              <div className="status-strip">
                <span className="status-pill">{humanizeLabel(selected.document.memoryScope)}</span>
                <span className="status-pill">{selected.document.namespace}</span>
                {selected.document.isPinned ? <span className="status-pill tone-sun">Pinned</span> : null}
              </div>
              <p>{selected.document.content}</p>
              <div className="stack-gap">
                {selected.chunks.map((chunk) => (
                  <article className="activity-card" key={chunk.id}>
                    <div className="activity-topline">
                      <strong>Chunk {chunk.ordinal + 1}</strong>
                      <span>{chunk.keywords.length} keyword(s)</span>
                    </div>
                    <p>{chunk.content}</p>
                  </article>
                ))}
              </div>
            </div>
          ) : (
            <div className="empty-state small">
              Select a session memory document to inspect its chunked detail.
            </div>
          )}
        </article>
      </section>
    </div>
  );
}
