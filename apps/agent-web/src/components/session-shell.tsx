"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import {
  getSession,
  listSessionArtifacts,
  listSessions,
  listTasks,
  updateSession,
} from "@/lib/api";
import { duplicateSessionWorkspace } from "@/lib/session-duplication";
import type {
  ArtifactRecord,
  SessionDetail,
  SessionRecord,
  TaskRecord,
} from "@/lib/types";
import { excerpt, formatModelLabel, humanizeLabel } from "@/lib/view";

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
  const [sessionRecord, setSessionRecord] = useState<SessionRecord | null>(null);
  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [artifacts, setArtifacts] = useState<ArtifactRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const [detailResult, tasksResult, artifactsResult, sessionsResult] =
      await Promise.allSettled([
        getSession(sessionId),
        listTasks(sessionId),
        listSessionArtifacts(sessionId),
        listSessions(),
      ]);

    setDetail(detailResult.status === "fulfilled" ? detailResult.value : null);
    setTasks(tasksResult.status === "fulfilled" ? tasksResult.value : []);
    setArtifacts(artifactsResult.status === "fulfilled" ? artifactsResult.value : []);
    setSessionRecord(
      sessionsResult.status === "fulfilled"
        ? sessionsResult.value.find((session) => session.id === sessionId) ?? null
        : null,
    );

    const rejectedResult = [detailResult, tasksResult, artifactsResult].find(
      (result) => result.status === "rejected",
    );

    setError(
      rejectedResult?.status === "rejected"
        ? rejectedResult.reason instanceof Error
          ? rejectedResult.reason.message
          : "Failed to load session workspace."
        : null,
    );
  }, [sessionId]);

  useEffect(() => {
    void refresh();

    const handleSessionUpdate = (event: Event) => {
      const payload = (event as CustomEvent<{ sessionId?: string }>).detail;
      if (payload?.sessionId === sessionId) {
        void refresh();
      }
    };

    window.addEventListener("asuka:session-updated", handleSessionUpdate);
    window.addEventListener("asuka:session-skills-updated", handleSessionUpdate);

    return () => {
      window.removeEventListener("asuka:session-updated", handleSessionUpdate);
      window.removeEventListener("asuka:session-skills-updated", handleSessionUpdate);
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

  const latestModelLabel = formatModelLabel(
    detail?.latestRunSummary?.selectedProvider,
    detail?.latestRunSummary?.selectedModel,
  );
  const displaySession = detail?.session ?? sessionRecord;

  return (
    <div className="session-stage">
      <section className="session-toolbar">
        <div className="session-toolbar-row">
          <div className="session-toolbar-main">
            <span className="workspace-title-icon">A</span>
            <div>
              <p className="eyebrow">Session</p>
              <h1>{displaySession?.title ?? "Session workspace"}</h1>
              <p>
                {displaySession?.summary
                  ? excerpt(displaySession.summary, 130)
                  : "This session owns its own transcript, runs, artifacts, memory, and settings."}
              </p>
            </div>
          </div>

          <div className="session-toolbar-meta">
            <span className="status-pill">
              {humanizeLabel(displaySession?.status ?? "loading")}
            </span>
            {latestModelLabel ? <span className="status-pill tone-sky">{latestModelLabel}</span> : null}
            <span className="status-pill">{tasks.length} tasks</span>
            <span className="status-pill">{artifacts.length} artifacts</span>
          </div>

          <div className="session-toolbar-actions">
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
              onClick={() => void handleDuplicate()}
              type="button"
            >
              Duplicate
            </button>
          </div>
        </div>

        <div className="session-toolbar-nav">
          {sessionNav.map((item) => {
            const href = `/sessions/${sessionId}${item.suffix}`;
            const isActive = pathname === href;

            return (
              <Link
                className={`workspace-tab${isActive ? " is-active" : ""}`}
                href={href}
                key={href}
              >
                {item.label}
              </Link>
            );
          })}
        </div>

        <div className="session-toolbar-feedback">
          {error ? <p className="error-copy">{error}</p> : null}
        </div>
      </section>

      <div className="session-page-slot">{children}</div>
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
