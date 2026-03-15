"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import {
  getSession,
  listSessionArtifacts,
  listTasks,
  updateSession,
} from "@/lib/api";
import { duplicateSessionWorkspace } from "@/lib/session-duplication";
import type {
  ArtifactRecord,
  SessionDetail,
  TaskRecord,
} from "@/lib/types";

type SessionShellProps = {
  sessionId: string;
  children: React.ReactNode;
};

const sessionNav = [
  { suffix: "/chat", label: "Chat" },
  { suffix: "/execution", label: "Execution" },
  { suffix: "/artifacts", label: "Artifacts" },
  { suffix: "/memory", label: "Memory" },
  { suffix: "/skills", label: "Skills" },
  { suffix: "/settings", label: "Settings" },
];

export function SessionShell({ sessionId, children }: SessionShellProps) {
  const pathname = usePathname();
  const router = useRouter();
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [artifacts, setArtifacts] = useState<ArtifactRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [railCollapsed, setRailCollapsed] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [nextDetail, nextTasks, nextArtifacts] = await Promise.all([
        getSession(sessionId),
        listTasks(sessionId),
        listSessionArtifacts(sessionId),
      ]);
      setDetail(nextDetail);
      setTasks(nextTasks);
      setArtifacts(nextArtifacts);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Failed to load session workspace.",
      );
    }
  }, [sessionId]);

  useEffect(() => {
    void refresh();

    const handleSkillUpdate = (event: Event) => {
      const detail = (event as CustomEvent<{ sessionId?: string }>).detail;
      if (detail?.sessionId === sessionId) {
        void refresh();
      }
    };

    const handleSessionUpdate = (event: Event) => {
      const detail = (event as CustomEvent<{ sessionId?: string }>).detail;
      if (detail?.sessionId === sessionId) {
        void refresh();
      }
    };

    window.addEventListener("asuka:session-skills-updated", handleSkillUpdate);
    window.addEventListener("asuka:session-updated", handleSessionUpdate);

    return () => {
      window.removeEventListener("asuka:session-skills-updated", handleSkillUpdate);
      window.removeEventListener("asuka:session-updated", handleSessionUpdate);
    };
  }, [refresh, sessionId]);

  async function handleRename() {
    if (!detail) {
      return;
    }

    const nextTitle = window.prompt("Rename session", detail.session.title);
    if (!nextTitle || nextTitle.trim() === detail.session.title) {
      return;
    }

    setBusyKey("rename");
    try {
      const updated = await updateSession(sessionId, {
        title: nextTitle.trim(),
      });
      setDetail((current) =>
        current
          ? {
              ...current,
              session: updated,
            }
          : current,
      );
      emitSessionUpdated(sessionId);
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to rename this session.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handleToggleArchive() {
    if (!detail) {
      return;
    }

    setBusyKey("archive");
    try {
      const updated = await updateSession(sessionId, {
        status: detail.session.status === "archived" ? "active" : "archived",
      });
      setDetail((current) =>
        current
          ? {
              ...current,
              session: updated,
            }
          : current,
      );
      emitSessionUpdated(sessionId);
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to update the session status.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handleDuplicate() {
    if (!detail) {
      return;
    }

    setBusyKey("duplicate");
    try {
      const duplicate = await duplicateSessionWorkspace(
        sessionId,
        `${detail.session.title} copy`,
      );
      setError(null);
      router.push(`/sessions/${duplicate.id}/chat`);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to duplicate this session.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  const latestRun = detail?.latestRunSummary ?? null;
  const activeRun = detail?.activeRunSummary ?? null;
  const activeTask = detail?.activeTaskSummary ?? null;
  const streamCheckpoint = detail?.latestStreamCheckpointSummary ?? null;
  const latestModelLabel =
    latestRun?.selectedProvider && latestRun?.selectedModel
      ? `${latestRun.selectedProvider} · ${latestRun.selectedModel}`
      : latestRun?.selectedModel ?? latestRun?.selectedProvider ?? null;
  const recentTasks = tasks.slice(0, 4);
  const recentArtifacts = artifacts.slice(0, 5);
  const pinnedNames = detail?.skillSummary.pinnedSkills.map((skill) => skill.name) ?? [];

  return (
    <div className="session-workspace">
      <aside className={`panel session-nav-rail${railCollapsed ? " is-collapsed" : ""}`}>
        <div className="session-rail-topline">
          <div>
            <p className="eyebrow">Workspace</p>
            <h2>{detail?.session.title ?? "Loading session"}</h2>
          </div>
          <button
            className="ghost-button"
            onClick={() => setRailCollapsed((current) => !current)}
            type="button"
          >
            {railCollapsed ? "Expand" : "Collapse"}
          </button>
        </div>

        {!railCollapsed ? (
          <>
            <p className="hint-copy">
              {detail?.session.summary ??
                "This session owns its own chat, execution history, artifacts, memory, and skills."}
            </p>

            <nav className="session-nav-list">
              {sessionNav.map((item) => {
                const href = `/sessions/${sessionId}${item.suffix}`;
                const isActive = pathname === href;

                return (
                  <Link
                    className={`session-nav-link${isActive ? " is-active" : ""}`}
                    href={href}
                    key={href}
                  >
                    {item.label}
                  </Link>
                );
              })}
            </nav>

            <div className="session-meta-card">
              <div className="status-strip">
                <span className="status-pill">{detail?.session.status ?? "loading"}</span>
                <span className="status-pill">
                  {detail?.skillSummary.effectiveSkillCount ?? 0} skill(s)
                </span>
              </div>
              {latestModelLabel ? <p className="hint-copy">{latestModelLabel}</p> : null}
              {activeRun ? (
                <div className="status-strip">
                  <span className="status-pill">{activeRun.status}</span>
                  <span className="status-pill">{activeRun.streamStatus}</span>
                </div>
              ) : null}
              {pinnedNames.length > 0 ? (
                <div className="session-chip-list">
                  {pinnedNames.map((name) => (
                    <span className="timeline-chip artifact" key={name}>
                      {name}
                    </span>
                  ))}
                </div>
              ) : null}
            </div>

            <div className="session-rail-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Recent tasks</p>
                  <h3>Session focus</h3>
                </div>
              </div>
              <div className="session-rail-list">
                {recentTasks.map((task) => (
                  <Link
                    className="session-rail-link"
                    href={`/sessions/${sessionId}/execution`}
                    key={task.id}
                  >
                    <strong>{task.title}</strong>
                    <span>{task.status}</span>
                  </Link>
                ))}
                {recentTasks.length === 0 ? (
                  <div className="empty-state small">
                    Post a message to create the first task in this workspace.
                  </div>
                ) : null}
              </div>
            </div>

            <div className="session-rail-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Recent artifacts</p>
                  <h3>Workspace outputs</h3>
                </div>
              </div>
              <div className="session-rail-list">
                {recentArtifacts.map((artifact) => (
                  <Link
                    className="session-rail-link"
                    href={`/sessions/${sessionId}/artifacts`}
                    key={artifact.id}
                  >
                    <strong>{artifact.displayName}</strong>
                    <span>{artifact.kind}</span>
                  </Link>
                ))}
                {recentArtifacts.length === 0 ? (
                  <div className="empty-state small">
                    Artifact previews appear here after a run writes workspace files.
                  </div>
                ) : null}
              </div>
            </div>
          </>
        ) : null}

        <div className="stack-inline">
          <Link className="ghost-button" href="/sessions">
            All sessions
          </Link>
          <Link className="ghost-button" href="/dashboard">
            Dashboard
          </Link>
        </div>
      </aside>

      <div className="session-content-shell">
        {error ? <p className="error-copy">{error}</p> : null}
        <section className="panel session-header-panel">
          <div className="session-header-main">
            <div>
              <p className="eyebrow">Session workspace</p>
              <h2>{detail?.session.title ?? "Loading session"}</h2>
            </div>
            <p className="hint-copy">
              {detail?.session.summary ??
                "Use the session routes to chat, inspect execution, browse artifacts, manage memory, and configure skills."}
            </p>
            <div className="session-header-meta">
              <span className="status-pill">{detail?.session.status ?? "loading"}</span>
              <span className="status-pill">
                {detail?.skillSummary.effectiveSkillCount ?? 0} effective skill(s)
              </span>
              {latestModelLabel ? <span className="status-pill">{latestModelLabel}</span> : null}
              {activeTask ? <span className="status-pill">{activeTask.title}</span> : null}
            </div>
            {streamCheckpoint ? (
              <div className="session-stream-summary">
                <strong>Recoverable stream checkpoint</strong>
                <p>
                  Run {streamCheckpoint.runId.slice(0, 8)} · seq{" "}
                  {streamCheckpoint.lastSequence}
                </p>
                <p>{streamCheckpoint.draftReplyText || "No draft deltas recorded yet."}</p>
              </div>
            ) : null}
          </div>

          <div className="session-header-actions">
            <Link className="primary-button" href={`/sessions/${sessionId}/chat`}>
              New task
            </Link>
            <button
              className="ghost-button"
              disabled={busyKey !== null}
              onClick={() => void handleRename()}
              type="button"
            >
              Rename
            </button>
            <button
              className="ghost-button"
              disabled={busyKey !== null}
              onClick={() => void handleToggleArchive()}
              type="button"
            >
              {detail?.session.status === "archived" ? "Restore" : "Archive"}
            </button>
            <button
              className="ghost-button"
              disabled={busyKey !== null}
              onClick={() => void handleDuplicate()}
              type="button"
            >
              Duplicate
            </button>
            <Link className="ghost-button" href={`/sessions/${sessionId}/settings`}>
              Settings
            </Link>
          </div>
        </section>

        <div className="session-page-slot">{children}</div>
      </div>
    </div>
  );
}

function emitSessionUpdated(sessionId: string) {
  window.dispatchEvent(
    new CustomEvent("asuka:session-updated", {
      detail: { sessionId },
    }),
  );
}
