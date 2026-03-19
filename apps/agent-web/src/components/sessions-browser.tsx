"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { createSession, listSessions } from "@/lib/api";
import type { SessionRecord } from "@/lib/types";
import { excerpt, formatDateTime, humanizeLabel } from "@/lib/view";

export function SessionsBrowser() {
  const router = useRouter();
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void listSessions()
      .then((nextSessions) => {
        if (!cancelled) {
          setSessions(nextSessions);
        }
      })
      .catch((loadError: unknown) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load sessions.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleCreateSession() {
    try {
      const session = await createSession("New workspace session");
      router.push(`/sessions/${session.id}/chat`);
    } catch (creationError) {
      setError(
        creationError instanceof Error
          ? creationError.message
          : "Failed to create a session.",
      );
    }
  }

  return (
    <div className="stack-gap">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Session Browser</p>
            <h2>Each workspace keeps its own chat, runs, artifacts, memory, and skills.</h2>
          </div>
          <p>
            Pick up a live thread, branch a workspace, or open a fresh session
            before you talk to the agent.
          </p>
          <div className="hero-actions">
            <button className="primary-button" onClick={handleCreateSession} type="button">
              Start new session
            </button>
            <Link className="ghost-button" href="/dashboard">
              Back to dashboard
            </Link>
          </div>
          {error ? <p className="error-copy">{error}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{sessions.length}</strong>
              <span>available workspaces</span>
            </article>
            <article className="hero-stat">
              <strong>{sessions.filter((session) => session.lastRunAt).length}</strong>
              <span>with run history</span>
            </article>
          </div>
          <article className="hero-orb">
            <p className="eyebrow">Why sessions matter</p>
            <strong>Frontend routes map directly onto backend session resources.</strong>
            <p>
              Selecting a session switches the UI into its scoped chat history,
              durable tasks, artifact tree, memory overview, and skill policy.
            </p>
          </article>
        </div>
      </section>

      <section className="session-browser-grid">
        {sessions.map((session) => (
          <article className="catalog-card" key={session.id}>
            <div className="text-block">
              <div className="story-meta">
                <p className="eyebrow">{humanizeLabel(session.status)}</p>
                <span className="status-pill tone-sky">
                  {session.lastRunAt ? "run history" : "empty"}
                </span>
              </div>
              <h3>{session.title}</h3>
              <p>{excerpt(session.summary, 160)}</p>
            </div>
            <div className="story-footer">
              <span className="story-kicker">Updated {formatDateTime(session.updatedAt)}</span>
              <span className="story-kicker">{session.id.slice(0, 8)}</span>
            </div>
            <div className="button-row">
              <Link className="primary-button" href={`/sessions/${session.id}/chat`}>
                Open chat
              </Link>
              <Link className="ghost-button" href={`/sessions/${session.id}/execution`}>
                Execution
              </Link>
              <Link className="ghost-button" href={`/sessions/${session.id}/skills`}>
                Skills
              </Link>
            </div>
          </article>
        ))}
        {sessions.length === 0 ? (
          <div className="panel empty-state">
            No sessions yet. Start one from the dashboard or create it here.
          </div>
        ) : null}
      </section>
    </div>
  );
}
