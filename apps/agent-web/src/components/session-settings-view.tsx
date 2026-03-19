"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import {
  getSession,
  getSessionMemoryOverview,
  getSessionSkills,
  updateSession,
} from "@/lib/api";
import { duplicateSessionWorkspace } from "@/lib/session-duplication";
import type {
  SessionDetail,
  SessionMemoryOverview,
  SessionSkillsDetail,
} from "@/lib/types";
import {
  compactId,
  excerpt,
  formatDateTime,
  humanizeLabel,
} from "@/lib/view";

type SessionSettingsViewProps = {
  sessionId: string;
};

export function SessionSettingsView({ sessionId }: SessionSettingsViewProps) {
  const router = useRouter();
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [skills, setSkills] = useState<SessionSkillsDetail | null>(null);
  const [memoryOverview, setMemoryOverview] = useState<SessionMemoryOverview | null>(null);
  const [titleDraft, setTitleDraft] = useState("");
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [nextDetail, nextSkills, nextMemory] = await Promise.all([
        getSession(sessionId),
        getSessionSkills(sessionId),
        getSessionMemoryOverview(sessionId),
      ]);
      setDetail(nextDetail);
      setSkills(nextSkills);
      setMemoryOverview(nextMemory);
      setTitleDraft(nextDetail.session.title);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Failed to load session settings.",
      );
    }
  }, [sessionId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleRename(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!titleDraft.trim()) {
      return;
    }

    setBusyKey("rename");
    try {
      const updated = await updateSession(sessionId, {
        title: titleDraft.trim(),
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
      setFeedback(`Renamed session to ${updated.title}.`);
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to rename the session.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handleToggleArchive() {
    if (!detail) {
      return;
    }

    const nextStatus = detail.session.status === "archived" ? "active" : "archived";
    setBusyKey("archive");
    try {
      const updated = await updateSession(sessionId, {
        status: nextStatus,
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
      setFeedback(
        nextStatus === "archived"
          ? "Archived this session workspace."
          : "Restored this session workspace.",
      );
      setError(null);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to update session status.",
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
      setFeedback(`Created duplicate workspace ${duplicate.title}.`);
      setError(null);
      router.push(`/sessions/${duplicate.id}/chat`);
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : "Failed to duplicate the session workspace.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  const latestRun = detail?.latestRunSummary ?? null;
  const activeRun = detail?.activeRunSummary ?? null;
  const streamCheckpoint = detail?.latestStreamCheckpointSummary ?? null;

  return (
    <div className="session-settings-layout">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Session settings</p>
            <h2>Rename, duplicate, archive, and inspect the workspace envelope.</h2>
          </div>
          <p>
            This page combines session detail, skill summary, memory overview,
            and active run metadata into one control surface.
          </p>
          {error ? <p className="error-copy">{error}</p> : null}
          {feedback ? <p className="status-pill tone-mint">{feedback}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{detail?.skillSummary.effectiveSkillCount ?? 0}</strong>
              <span>effective skills</span>
            </article>
            <article className="hero-stat">
              <strong>{memoryOverview?.scopedDocuments.length ?? 0}</strong>
              <span>scoped notes</span>
            </article>
          </div>
        </div>
      </section>

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Workspace controls</p>
            <h2>Core session actions</h2>
          </div>
          <div className="stack-inline">
            <Link className="ghost-button" href={`/sessions/${sessionId}/chat`}>
              New task in chat
            </Link>
            <button
              className="primary-button"
              disabled={busyKey !== null}
              onClick={() => void handleDuplicate()}
              type="button"
            >
              Duplicate session
            </button>
          </div>
        </div>

        <form className="session-settings-form stack-gap" onSubmit={handleRename}>
          <label>
            Session title
            <input
              className="text-input"
              onChange={(event) => setTitleDraft(event.target.value)}
              value={titleDraft}
            />
          </label>
          <div className="button-row">
            <button
              className="ghost-button"
              disabled={busyKey !== null}
              onClick={() => void handleToggleArchive()}
              type="button"
            >
              {detail?.session.status === "archived" ? "Restore session" : "Archive session"}
            </button>
            <button className="primary-button" disabled={busyKey !== null} type="submit">
              Save title
            </button>
          </div>
        </form>

        <div className="session-settings-meta-grid">
          <article className="session-settings-card">
            <p className="eyebrow">Workspace</p>
            <div className="status-strip">
              <span className="status-pill tone-sun">
                {humanizeLabel(detail?.session.status ?? "loading")}
              </span>
              <span className="status-pill tone-sky">
                {detail?.skillSummary.effectiveSkillCount ?? 0} skill(s)
              </span>
              <span className="status-pill tone-mint">
                {memoryOverview?.scopedDocuments.length ?? 0} scoped notes
              </span>
            </div>
            <p>
              {detail?.session.summary ??
                "This session controls its own chat history, artifacts, skills, and memory scope."}
            </p>
          </article>

          <article className="session-settings-card">
            <p className="eyebrow">Skills</p>
            <p>
              Policy: {humanizeLabel(skills?.policy.mode ?? "inheritDefault")}
              {skills?.policy.presetId ? ` · ${skills.policy.presetId}` : ""}
            </p>
            <div className="session-chip-list">
              {detail?.skillSummary.pinnedSkills.map((skill) => (
                <span className="timeline-chip artifact" key={skill.id}>
                  {skill.name}
                </span>
              ))}
              {detail?.skillSummary.pinnedSkills.length === 0 ? (
                <span className="story-kicker">No pinned session skills.</span>
              ) : null}
            </div>
          </article>
        </div>
      </section>

      <section className="session-settings-layout-grid">
        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Run status</p>
              <h2>Active and latest execution</h2>
            </div>
          </div>

          <div className="session-settings-card-list">
            <article className="session-settings-card">
              <p className="eyebrow">Active run</p>
              {activeRun ? (
                <>
                  <div className="status-strip">
                    <span className="status-pill">{humanizeLabel(activeRun.status)}</span>
                    <span className="status-pill">{humanizeLabel(activeRun.streamStatus)}</span>
                  </div>
                  <p>
                    {activeRun.selectedProvider ?? "Local runtime"} ·{" "}
                    {activeRun.selectedModel ?? "fallback"}
                  </p>
                  <p className="hint-copy">Event seq {activeRun.lastEventSequence}</p>
                </>
              ) : (
                <p className="hint-copy">No run is currently active in this session.</p>
              )}
            </article>

            <article className="session-settings-card">
              <p className="eyebrow">Latest run</p>
              {latestRun ? (
                <>
                  <div className="status-strip">
                    <span className="status-pill">{humanizeLabel(latestRun.status)}</span>
                    {latestRun.selectedProvider ? (
                      <span className="status-pill">{latestRun.selectedProvider}</span>
                    ) : null}
                  </div>
                  <p>
                    {latestRun.selectedModel ?? "fallback"} · {formatDateTime(latestRun.startedAt)}
                  </p>
                  <div className="session-chip-list">
                    {latestRun.pinnedSkillNames.map((skillName) => (
                      <span className="timeline-chip artifact" key={skillName}>
                        {skillName}
                      </span>
                    ))}
                  </div>
                </>
              ) : (
                <p className="hint-copy">No completed run has been recorded yet.</p>
              )}
            </article>

            <article className="session-settings-card">
              <p className="eyebrow">Stream checkpoint</p>
              {streamCheckpoint ? (
                <>
                  <div className="status-strip">
                    <span className="status-pill">Run {compactId(streamCheckpoint.runId)}</span>
                    <span className="status-pill">Seq {streamCheckpoint.lastSequence}</span>
                  </div>
                  <p>{excerpt(streamCheckpoint.draftReplyText, 180)}</p>
                </>
              ) : (
                <p className="hint-copy">
                  Stream checkpoint data appears here while a run is being recovered.
                </p>
              )}
            </article>
          </div>
        </article>

        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Task focus</p>
              <h2>Current workspace context</h2>
            </div>
          </div>

          <article className="session-settings-card">
            <p className="eyebrow">Active task</p>
            {detail?.activeTaskSummary ? (
              <>
                <div className="status-strip">
                  <span className="status-pill">{humanizeLabel(detail.activeTaskSummary.status)}</span>
                  <span className="status-pill">
                    Task {compactId(detail.activeTaskSummary.id)}
                  </span>
                </div>
                <strong>{detail.activeTaskSummary.title}</strong>
                <p>{excerpt(detail.activeTaskSummary.summary, 160)}</p>
              </>
            ) : (
              <p className="hint-copy">
                The session has no active task yet. Posting a new message will create one.
              </p>
            )}
          </article>

          <article className="session-settings-card">
            <p className="eyebrow">Scoped memory</p>
            <p>
              {memoryOverview?.scopedDocuments.length ?? 0} session-scoped documents,{" "}
              {memoryOverview?.pinnedDocuments.length ?? 0} pinned.
            </p>
            <div className="button-row">
              <Link className="ghost-button" href={`/sessions/${sessionId}/memory`}>
                Open session memory
              </Link>
              <Link className="ghost-button" href={`/sessions/${sessionId}/skills`}>
                Configure skills
              </Link>
            </div>
          </article>
        </article>
      </section>
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
