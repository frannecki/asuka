"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import {
  createSession,
  listSessionArtifacts,
  listSessions,
  listTasks,
} from "@/lib/api";
import type { ArtifactRecord, SessionRecord, TaskRecord } from "@/lib/types";
import { compactId, excerpt, formatDateTime, humanizeLabel } from "@/lib/view";

type SessionStatusRow = {
  session: SessionRecord;
  tasks: TaskRecord[];
  artifacts: ArtifactRecord[];
};

export function DashboardHome() {
  const router = useRouter();
  const [rows, setRows] = useState<SessionStatusRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const sessions = await listSessions();
        const taskMatrix = await Promise.all(
          sessions.map(async (session) => ({
            session,
            tasks: await listTasks(session.id),
            artifacts: await listSessionArtifacts(session.id),
          })),
        );

        if (!cancelled) {
          setRows(taskMatrix);
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(
            loadError instanceof Error
              ? loadError.message
              : "Failed to load dashboard data.",
          );
        }
      }
    }

    void load();

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

  const totalSessions = rows.length;
  const activeTasks = rows.flatMap((row) => row.tasks).filter((task) =>
    ["queued", "planning", "running", "waitingForApproval", "suspended"].includes(task.status),
  ).length;
  const totalArtifacts = rows.reduce((sum, row) => sum + row.artifacts.length, 0);

  return (
    <div className="dashboard-board">
      <aside className="dashboard-summary">
        <div className="dashboard-summary-copy">
          <p className="eyebrow">Dashboard</p>
          <h1>All sessions status</h1>
          <p>
            A single overview of session health, task activity, and artifact output.
          </p>
        </div>
        <div className="dashboard-summary-stats">
          <div className="dashboard-summary-stat">
            <strong>{totalSessions}</strong>
            <span>Sessions</span>
          </div>
          <div className="dashboard-summary-stat">
            <strong>{activeTasks}</strong>
            <span>Active tasks</span>
          </div>
          <div className="dashboard-summary-stat">
            <strong>{totalArtifacts}</strong>
            <span>Artifacts</span>
          </div>
        </div>
        <div className="dashboard-summary-actions">
          <button className="primary-button" onClick={handleCreateSession} type="button">
            New session
          </button>
        </div>
      </aside>

      <section className="dashboard-stream">
        <div className="dashboard-stream-head">
          <div>
            <p className="eyebrow">Session feed</p>
            <h2>Recent workspaces</h2>
          </div>
          <span className="status-pill tone-sky">{rows.length} loaded</span>
        </div>

        {error ? <p className="error-copy">{error}</p> : null}

        <div className="dashboard-stream-list">
          {rows.map(({ session, tasks, artifacts }) => {
            const latestTask = tasks[0] ?? null;
            return (
              <article className="dashboard-stream-row" key={session.id}>
                <div className="dashboard-stream-main">
                  <strong>{session.title}</strong>
                  <p>{excerpt(session.summary, 88)}</p>
                </div>
                <div className="dashboard-stream-task">
                  <p className="eyebrow">Latest task</p>
                  {latestTask ? (
                    <>
                      <strong>{excerpt(latestTask.title, 38)}</strong>
                      <p>{humanizeLabel(latestTask.status)} · {compactId(latestTask.id)}</p>
                    </>
                  ) : (
                    <p>No tasks yet</p>
                  )}
                </div>
                <div className="dashboard-stream-meta">
                  <span className="status-pill">{humanizeLabel(session.status)}</span>
                  <span>{artifacts.length} files</span>
                  <p>{formatDateTime(session.updatedAt)}</p>
                  <Link className="ghost-button" href={`/sessions/${session.id}/chat`}>
                    Open
                  </Link>
                </div>
              </article>
            );
          })}
        </div>
      </section>
    </div>
  );
}
