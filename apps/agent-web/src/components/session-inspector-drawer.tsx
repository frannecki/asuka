"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { describeRunEvent } from "@/components/chat-activity";
import {
  buildSessionWorkspaceRawUrl,
  buildSessionWorkspaceRenderUrl,
} from "@/lib/api";
import type {
  ArtifactRecord,
  RunEventEnvelope,
  RunStepRecord,
  TaskRecord,
  ToolInvocationRecord,
} from "@/lib/types";

type SessionInspectorDrawerProps = {
  sessionId: string | null;
  activeTaskId: string | null;
  tasks: TaskRecord[];
  runSteps: RunStepRecord[];
  toolInvocations: ToolInvocationRecord[];
  activity: RunEventEnvelope[];
  artifacts: ArtifactRecord[];
  selectedPath: string | null;
  onSelectPath: (path: string) => void;
  onSelectTaskId: (taskId: string) => void;
  modelLabel: string | null;
  status: string;
};

type InspectorTab = "activity" | "run" | "artifacts";
type PreviewMode = "markdown" | "html" | "text" | "empty";

export function SessionInspectorDrawer({
  sessionId,
  activeTaskId,
  tasks,
  runSteps,
  toolInvocations,
  activity,
  artifacts,
  selectedPath,
  onSelectPath,
  onSelectTaskId,
  modelLabel,
  status,
}: SessionInspectorDrawerProps) {
  const [tab, setTab] = useState<InspectorTab>("activity");
  const [textPreview, setTextPreview] = useState("");
  const [previewError, setPreviewError] = useState<string | null>(null);

  const activeTask = tasks.find((task) => task.id === activeTaskId) ?? tasks[0] ?? null;
  const visibleArtifacts = activeTaskId
    ? artifacts.filter((artifact) => artifact.taskId === activeTaskId)
    : artifacts;
  const selectedArtifact =
    visibleArtifacts.find((artifact) => artifact.path === selectedPath) ??
    visibleArtifacts[0] ??
    null;
  const previewMode = getArtifactPreviewMode(selectedArtifact);
  const recentActivity = activity.slice(-8).reverse();
  const recentSteps = runSteps.slice(-6).reverse();
  const recentToolInvocations = toolInvocations.slice(-4).reverse();

  useEffect(() => {
    if (!selectedArtifact || !sessionId || previewMode !== "text") {
      return;
    }

    let cancelled = false;

    void fetch(buildSessionWorkspaceRawUrl(sessionId, selectedArtifact.path), {
      cache: "no-store",
    })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`Workspace preview failed with ${response.status}`);
        }

        return response.text();
      })
      .then((content) => {
        if (cancelled) {
          return;
        }

        setTextPreview(content);
        setPreviewError(null);
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }

        setPreviewError(
          error instanceof Error ? error.message : "Failed to preview workspace file.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [previewMode, selectedArtifact, sessionId]);

  return (
    <aside className="panel session-inspector-drawer">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Inspector</p>
          <h2>Contextual run details</h2>
        </div>
        <div className="stack-inline">
          {modelLabel ? <span className="status-pill">{modelLabel}</span> : null}
          <span className="status-pill">{status}</span>
        </div>
      </div>

      <div className="inspector-tab-row">
        {([
          ["activity", "Activity"],
          ["run", "Latest run"],
          ["artifacts", "Artifacts"],
        ] as const).map(([value, label]) => (
          <button
            className={`policy-chip${tab === value ? " is-active" : ""}`}
            key={value}
            onClick={() => setTab(value)}
            type="button"
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "activity" ? (
        <div className="session-drawer-pane">
          <article className="drawer-summary-card">
            <div className="status-strip">
              <span className="status-pill">{status}</span>
              {activeTask ? <span className="status-pill">{activeTask.title}</span> : null}
            </div>
            <p>
              {activeTask?.summary ??
                "Active stream events, fallback notices, and memory/tool activity appear here."}
            </p>
          </article>

          <div className="activity-list compact">
            {recentActivity.map((event) => {
              const descriptor = describeRunEvent(event);

              return (
                <article
                  className={`activity-card tone-${descriptor.tone}`}
                  key={event.sequence}
                >
                  <div className="activity-topline">
                    <span className="activity-badge">{descriptor.badge}</span>
                    <span>{new Date(event.timestamp).toLocaleTimeString()}</span>
                  </div>
                  <div className="activity-copy">
                    <strong>{descriptor.title}</strong>
                    <p>{descriptor.summary}</p>
                  </div>
                </article>
              );
            })}
            {recentActivity.length === 0 ? (
              <div className="empty-state small">
                Streamed lifecycle events will appear here during active runs.
              </div>
            ) : null}
          </div>
        </div>
      ) : null}

      {tab === "run" ? (
        <div className="session-drawer-pane">
          {tasks.length > 0 ? (
            <div className="task-selector compact">
              {tasks.slice(0, 5).map((task) => (
                <button
                  className={`task-chip${task.id === activeTask?.id ? " is-active" : ""}`}
                  key={task.id}
                  onClick={() => onSelectTaskId(task.id)}
                  type="button"
                >
                  <strong>{task.title}</strong>
                  <span>{task.status}</span>
                </button>
              ))}
            </div>
          ) : null}

          <article className="drawer-summary-card">
            <div className="status-strip">
              {activeTask ? <span className="status-pill">{activeTask.status}</span> : null}
              {activeTask?.latestRunId ? (
                <span className="status-pill">run {activeTask.latestRunId.slice(0, 8)}</span>
              ) : null}
            </div>
            <strong>{activeTask?.title ?? "No active task selected"}</strong>
            <p>
              {activeTask?.summary ??
                "Open the execution view for the full timeline, graph, and artifact groups."}
            </p>
            {sessionId ? (
              <Link className="ghost-button" href={`/sessions/${sessionId}/execution`}>
                Open execution view
              </Link>
            ) : null}
          </article>

          <div className="drawer-run-section">
            <div className="panel-header">
              <div>
                <p className="eyebrow">Run steps</p>
                <h3>Recent durable steps</h3>
              </div>
            </div>
            <div className="run-step-list compact">
              {recentSteps.map((step) => (
                <article className="run-step-card compact" key={step.id}>
                  <div className="activity-topline">
                    <span className="activity-badge">#{step.sequence}</span>
                    <span>{step.status}</span>
                  </div>
                  <div className="activity-copy">
                    <strong>{step.title}</strong>
                    <p>{step.inputSummary}</p>
                  </div>
                </article>
              ))}
              {recentSteps.length === 0 ? (
                <div className="empty-state small">
                  Run steps will appear after the task starts executing.
                </div>
              ) : null}
            </div>
          </div>

          <div className="drawer-run-section">
            <div className="panel-header">
              <div>
                <p className="eyebrow">Tools</p>
                <h3>Latest invocations</h3>
              </div>
            </div>
            <div className="tool-invocation-list compact">
              {recentToolInvocations.map((invocation) => (
                <article className="tool-invocation-card compact" key={invocation.id}>
                  <div className="activity-topline">
                    <span className="activity-badge">{invocation.toolName}</span>
                    <span>{invocation.ok ? "ok" : "error"}</span>
                  </div>
                  <pre>{JSON.stringify(invocation.argumentsJson, null, 2)}</pre>
                </article>
              ))}
              {recentToolInvocations.length === 0 ? (
                <div className="empty-state small">
                  Persisted tool invocations will appear here once the runtime uses tools.
                </div>
              ) : null}
            </div>
          </div>
        </div>
      ) : null}

      {tab === "artifacts" ? (
        <div className="session-drawer-pane">
          <div className="drawer-artifact-list">
            {visibleArtifacts.slice(0, 6).map((artifact) => (
              <button
                className={`artifact-card${selectedArtifact?.id === artifact.id ? " is-active" : ""}`}
                key={artifact.id}
                onClick={() => onSelectPath(artifact.path)}
                type="button"
              >
                <header>
                  <span className={`artifact-kind artifact-${artifact.kind}`}>
                    {artifact.kind}
                  </span>
                  <strong>{artifact.displayName}</strong>
                </header>
                <p>{artifact.description}</p>
              </button>
            ))}
            {visibleArtifacts.length === 0 ? (
              <div className="empty-state small">
                Recent artifacts appear here after completed runs emit workspace files.
              </div>
            ) : null}
          </div>

          <article className="drawer-summary-card">
            <div className="panel-header">
              <div>
                <p className="eyebrow">Preview</p>
                <h3>{selectedArtifact?.displayName ?? "No artifact selected"}</h3>
              </div>
              {sessionId ? (
                <Link className="ghost-button" href={`/sessions/${sessionId}/artifacts`}>
                  Full browser
                </Link>
              ) : null}
            </div>

            {selectedArtifact ? (
              <>
                <div className="status-strip">
                  <span className="status-pill">{selectedArtifact.kind}</span>
                  <span className="status-pill">{selectedArtifact.renderMode}</span>
                </div>
                <p>{selectedArtifact.path}</p>
              </>
            ) : (
              <p className="hint-copy">
                Select an artifact to preview markdown, HTML, JSON, or text outputs.
              </p>
            )}
          </article>

          {selectedArtifact && sessionId && previewMode === "markdown" ? (
            <iframe
              className="session-drawer-frame"
              src={buildSessionWorkspaceRenderUrl(sessionId, selectedArtifact.path)}
              title={selectedArtifact.displayName}
            />
          ) : null}

          {selectedArtifact && sessionId && previewMode === "html" ? (
            <iframe
              className="session-drawer-frame"
              src={buildSessionWorkspaceRawUrl(sessionId, selectedArtifact.path)}
              title={selectedArtifact.displayName}
            />
          ) : null}

          {previewMode === "text" ? (
            previewError ? (
              <p className="error-copy">{previewError}</p>
            ) : (
              <pre className="workspace-text-preview session-drawer-text-preview">
                {textPreview}
              </pre>
            )
          ) : null}
        </div>
      ) : null}
    </aside>
  );
}

function getArtifactPreviewMode(artifact: ArtifactRecord | null): PreviewMode {
  if (!artifact) {
    return "empty";
  }
  if (artifact.renderMode === "markdown") {
    return "markdown";
  }
  if (artifact.renderMode === "html") {
    return "html";
  }
  return "text";
}
