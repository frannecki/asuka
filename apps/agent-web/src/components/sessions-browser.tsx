"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { createSession, listSessions } from "@/lib/api";
import type { SessionRecord } from "@/lib/types";

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
      <section className="panel dashboard-hero">
        <div className="dashboard-hero-copy">
          <p className="eyebrow">Sessions</p>
          <h2>Choose a workspace before you chat.</h2>
          <p>
            Each session now owns its own workspace routes, skill policy, and
            durable harness history.
          </p>
        </div>
        <div className="dashboard-hero-actions">
          <button className="primary-button" onClick={handleCreateSession} type="button">
            New session
          </button>
          <Link className="ghost-button" href="/dashboard">
            Back to dashboard
          </Link>
        </div>
      </section>

      {error ? <p className="error-copy">{error}</p> : null}

      <section className="session-browser-grid">
        {sessions.map((session) => (
          <article className="panel session-browser-card" key={session.id}>
            <div className="stack-gap">
              <div>
                <p className="eyebrow">{session.status}</p>
                <h2>{session.title}</h2>
              </div>
              <p>{session.summary}</p>
            </div>
            <div className="status-strip">
              <span className="status-pill">
                {session.lastRunAt
                  ? `last run ${new Date(session.lastRunAt).toLocaleString()}`
                  : "no runs yet"}
              </span>
            </div>
            <div className="stack-inline">
              <Link className="primary-button" href={`/sessions/${session.id}/chat`}>
                Open chat
              </Link>
              <Link className="ghost-button" href={`/sessions/${session.id}/skills`}>
                Configure skills
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
