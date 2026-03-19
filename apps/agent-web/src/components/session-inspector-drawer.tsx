"use client";

import Link from "next/link";

import { describeRunEvent } from "@/components/chat-activity";
import type {
  ArtifactRecord,
  RunEventEnvelope,
  RunStepRecord,
  TaskRecord,
  ToolInvocationRecord,
} from "@/lib/types";
import { compactId, excerpt, formatTime, humanizeLabel } from "@/lib/view";

type SessionInspectorDrawerProps = {
  sessionId: string | null;
  activeTaskId: string | null;
  tasks: TaskRecord[];
  runSteps: RunStepRecord[];
  toolInvocations: ToolInvocationRecord[];
  activity: RunEventEnvelope[];
  artifacts: ArtifactRecord[];
  isLoading: boolean;
  selectedPath: string | null;
  onSelectPath: (path: string) => void;
  onSelectTaskId: (taskId: string) => void;
  modelLabel: string | null;
  status: string;
};

export function SessionInspectorDrawer({
  sessionId,
  activeTaskId,
  tasks,
  runSteps,
  toolInvocations,
  activity,
  artifacts,
  isLoading,
  selectedPath,
  onSelectPath,
  onSelectTaskId,
  modelLabel,
  status,
}: SessionInspectorDrawerProps) {
  const activeTask = tasks.find((task) => task.id === activeTaskId) ?? tasks[0] ?? null;
  const recentActivity = activity.slice(0, 5);
  const recentArtifacts = artifacts.slice(0, 4);
  const currentRunLabel =
    modelLabel ?? (runSteps.length > 0 ? "Latest execution loaded" : "Waiting for model selection");

  return (
    <aside className="command-rail">
      <div className="rail-head">
        <div>
          <p className="eyebrow">Run monitor</p>
          <h2>Session pulse</h2>
        </div>
        <span className="status-pill tone-sun">{humanizeLabel(status)}</span>
      </div>

      <div className="rail-summary">
        <div className="rail-summary-item">
          <span>Model</span>
          <strong>{currentRunLabel}</strong>
        </div>
        <div className="rail-summary-item">
          <span>Status</span>
          <strong>{humanizeLabel(activeTask?.status ?? status)}</strong>
        </div>
        <div className="rail-summary-item">
          <span>Steps</span>
          <strong>{runSteps.length}</strong>
        </div>
        <div className="rail-summary-item">
          <span>Tools</span>
          <strong>{toolInvocations.length}</strong>
        </div>
      </div>

      <section className="rail-section">
        <div className="rail-section-head">
          <div>
            <p className="eyebrow">Focus</p>
            <h3>{activeTask?.title ?? "No active task"}</h3>
          </div>
        </div>
        <p className="rail-copy">
          {activeTask
            ? excerpt(activeTask.summary || activeTask.goal, 120)
            : "Send a prompt to create a task and start a run."}
        </p>
        <div className="rail-task-list">
          {isLoading ? (
            <div className="rail-loading-grid" aria-hidden="true">
              <div className="rail-loading-chip" />
              <div className="rail-loading-chip" />
              <div className="rail-loading-chip" />
            </div>
          ) : (
            tasks.slice(0, 4).map((task) => (
              <button
                className={`rail-task-button${task.id === activeTask?.id ? " is-active" : ""}`}
                key={task.id}
                onClick={() => onSelectTaskId(task.id)}
                type="button"
              >
                {excerpt(task.title, 26)}
              </button>
            ))
          )}
        </div>
      </section>

      <section className="rail-section">
        <div className="rail-section-head">
          <div>
            <p className="eyebrow">Recent activity</p>
            <h3>Live events</h3>
          </div>
        </div>
        <div className="rail-feed">
          {isLoading ? (
            <RailLoadingRows />
          ) : recentActivity.length > 0 ? (
            recentActivity.map((event) => {
              const descriptor = describeRunEvent(event);
              return (
                <div className="rail-feed-item" key={event.sequence}>
                  <div className="rail-feed-meta">
                    <span className="activity-badge">{descriptor.badge}</span>
                    <span>{formatTime(event.timestamp)}</span>
                  </div>
                  <strong>{descriptor.title}</strong>
                  <p>{descriptor.summary}</p>
                </div>
              );
            })
          ) : (
            <div className="empty-state small">
              Stream events will appear here while a run is active.
            </div>
          )}
        </div>
      </section>

      <section className="rail-section">
        <div className="rail-section-head">
          <div>
            <p className="eyebrow">Artifacts</p>
            <h3>Latest outputs</h3>
          </div>
          {sessionId ? (
            <Link className="ghost-button" href={`/sessions/${sessionId}/artifacts`}>
              Open
            </Link>
          ) : null}
        </div>
        <div className="rail-feed">
          {isLoading ? (
            <RailLoadingRows />
          ) : recentArtifacts.length > 0 ? (
            recentArtifacts.map((artifact) => (
              <button
                className={`rail-artifact${
                  selectedPath === artifact.path ? " is-active" : ""
                }`}
                key={artifact.id}
                onClick={() => onSelectPath(artifact.path)}
                type="button"
              >
                <div className="rail-feed-meta">
                  <span className={`artifact-kind artifact-${artifact.kind}`}>
                    {artifact.kind}
                  </span>
                  <span>{compactId(artifact.runId)}</span>
                </div>
                <strong>{artifact.displayName}</strong>
                <p>{excerpt(artifact.description, 82)}</p>
              </button>
            ))
          ) : (
            <div className="empty-state small">
              Artifacts appear here after the run writes workspace output.
            </div>
          )}
        </div>
      </section>
    </aside>
  );
}

function RailLoadingRows() {
  return (
    <div className="rail-loading-stack" aria-hidden="true">
      <div className="rail-loading-row" />
      <div className="rail-loading-row" />
      <div className="rail-loading-row short" />
    </div>
  );
}
