"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

import {
  buildSessionWorkspaceRawUrl,
  buildSessionWorkspaceRenderUrl,
} from "@/lib/api";
import type { ArtifactRecord, TaskRecord, WorkspaceNode } from "@/lib/types";
import {
  compactId,
  excerpt,
  formatArtifactSize,
  formatDateTime,
  humanizeLabel,
} from "@/lib/view";

type WorkspacePanelProps = {
  artifacts: ArtifactRecord[];
  error: string | null;
  sessionId: string | null;
  tasks: TaskRecord[];
  tree: WorkspaceNode | null;
  selectedPath: string | null;
  selectedTaskId: string | null;
  onSelectPath: (path: string) => void;
  onSelectTaskId: (taskId: string | null) => void;
};

type PreviewMode = "directory" | "markdown" | "html" | "text" | "unsupported" | "empty";

export function WorkspacePanel({
  artifacts,
  error,
  sessionId,
  tasks,
  tree,
  selectedPath,
  selectedTaskId,
  onSelectPath,
  onSelectTaskId,
}: WorkspacePanelProps) {
  const [textPreview, setTextPreview] = useState<string>("");
  const [previewError, setPreviewError] = useState<string | null>(null);

  const selectedTask = useMemo(
    () => tasks.find((task) => task.id === selectedTaskId) ?? null,
    [selectedTaskId, tasks],
  );
  const selectedNode = useMemo(
    () => (tree && selectedPath ? findWorkspaceNode(tree, selectedPath) : null),
    [tree, selectedPath],
  );
  const selectedArtifact = useMemo(
    () => artifacts.find((artifact) => artifact.path === selectedPath) ?? null,
    [artifacts, selectedPath],
  );
  const previewMode = getPreviewMode(selectedNode, selectedArtifact);
  const previewLabel = describePreviewMode(previewMode);
  const rawHref =
    sessionId && selectedPath ? buildSessionWorkspaceRawUrl(sessionId, selectedPath) : null;
  const renderedHref =
    sessionId && selectedPath && (previewMode === "markdown" || previewMode === "html")
      ? previewMode === "markdown"
        ? buildSessionWorkspaceRenderUrl(sessionId, selectedPath)
        : buildSessionWorkspaceRawUrl(sessionId, selectedPath)
      : null;
  const workspaceFileCount = useMemo(() => countWorkspaceFiles(tree), [tree]);

  useEffect(() => {
    if (previewMode !== "text" || !sessionId || !selectedPath) {
      return;
    }

    let cancelled = false;

    void fetch(buildSessionWorkspaceRawUrl(sessionId, selectedPath), {
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
      .catch((nextError: unknown) => {
        if (cancelled) {
          return;
        }

        setPreviewError(
          nextError instanceof Error ? nextError.message : "Failed to preview workspace file.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [previewMode, selectedPath, sessionId]);

  return (
    <div className="workspace-shell-outer stack-gap">
      <section className="artifact-shell">
        <aside className="artifact-sidebar">
          <div className="artifact-sidebar-head">
            <div>
              <p className="eyebrow">Artifact index</p>
              <h2>Workspace outputs</h2>
            </div>
            <span className="status-pill tone-sky">{artifacts.length} files</span>
          </div>

          {tasks.length > 0 ? (
            <section className="artifact-sidebar-section">
              <div className="artifact-section-head">
                <div>
                  <p className="eyebrow">Task lane</p>
                  <h3>{selectedTask ? excerpt(selectedTask.title, 30) : "All outputs"}</h3>
                </div>
              </div>
              <div className="artifact-task-filter">
                <button
                  className={`rail-task-button${selectedTaskId === null ? " is-active" : ""}`}
                  onClick={() => onSelectTaskId(null)}
                  type="button"
                >
                  All outputs
                </button>
                {tasks.map((task) => (
                  <button
                    className={`rail-task-button${
                      task.id === selectedTaskId ? " is-active" : ""
                    }`}
                    key={task.id}
                    onClick={() => onSelectTaskId(task.id)}
                    type="button"
                  >
                    {excerpt(task.title, 24)}
                  </button>
                ))}
              </div>
            </section>
          ) : null}

          <section className="artifact-sidebar-section is-fill">
            <div className="artifact-section-head">
              <div>
                <p className="eyebrow">Artifacts</p>
                <h3>Durable files</h3>
              </div>
              <span className="artifact-section-count">{artifacts.length}</span>
            </div>
            <div className="artifact-list-scroll">
              {artifacts.length > 0 ? (
                artifacts.map((artifact) => (
                  <button
                    className={`artifact-list-item${
                      selectedPath === artifact.path ? " is-active" : ""
                    }`}
                    key={artifact.id}
                    onClick={() => onSelectPath(artifact.path)}
                    type="button"
                  >
                    <div className="artifact-list-item-head">
                      <span className={`artifact-kind artifact-${artifact.kind}`}>
                        {artifact.kind}
                      </span>
                      <span className="artifact-item-size">
                        {formatArtifactSize(artifact.sizeBytes)}
                      </span>
                    </div>
                    <strong>{artifact.displayName}</strong>
                    <p>{excerpt(artifact.description, 92)}</p>
                    <div className="artifact-item-meta">
                      <span>{compactId(artifact.runId)}</span>
                      <span>{artifact.renderMode}</span>
                    </div>
                  </button>
                ))
              ) : (
                <div className="empty-state small">
                  Completed runs will register durable reports, markdown files,
                  and JSON outputs here.
                </div>
              )}
            </div>
          </section>

          <section className="artifact-sidebar-section">
            <div className="artifact-section-head">
              <div>
                <p className="eyebrow">Workspace tree</p>
                <h3>All files</h3>
              </div>
              <span className="artifact-section-count">{workspaceFileCount}</span>
            </div>
            <div className="artifact-tree-scroll">
              {tree?.children.length ? (
                tree.children.map((node) => (
                  <WorkspaceTreeNode
                    key={node.path || node.name}
                    node={node}
                    onSelectPath={onSelectPath}
                    selectedPath={selectedPath}
                  />
                ))
              ) : (
                <div className="empty-state small">
                  The workspace tree will populate as soon as the runtime writes
                  files into this session.
                </div>
              )}
            </div>
          </section>
        </aside>

        <section className="artifact-main">
          <div className="artifact-main-head">
            <div className="artifact-title-block">
              <p className="eyebrow">Preview canvas</p>
              <h2>{selectedArtifact?.displayName ?? selectedNode?.name ?? "Select an output"}</h2>
              <p>
                {selectedArtifact
                  ? excerpt(selectedArtifact.description, 160)
                  : selectedNode?.path ??
                    "Select an artifact or workspace file to inspect its contents."}
              </p>
              {error ? <p className="error-copy command-error-inline">{error}</p> : null}
            </div>
            <div className="artifact-stats">
              <div className="command-stat">
                <span>Selection</span>
                <strong>
                  {selectedArtifact
                    ? humanizeLabel(selectedArtifact.kind)
                    : selectedNode
                      ? humanizeLabel(selectedNode.kind)
                      : "Waiting"}
                </strong>
              </div>
              <div className="command-stat">
                <span>Preview</span>
                <strong>{previewLabel}</strong>
              </div>
              <div className="command-stat">
                <span>Task</span>
                <strong>
                  {selectedArtifact
                    ? compactId(selectedArtifact.taskId)
                    : selectedTask
                      ? compactId(selectedTask.id)
                      : "All"}
                </strong>
              </div>
              <div className="command-stat">
                <span>Outputs</span>
                <strong>{artifacts.length}</strong>
              </div>
            </div>
          </div>

          <div className="artifact-preview-board">
            <div className="artifact-preview-toolbar">
              {selectedArtifact ? (
                <span className={`artifact-kind artifact-${selectedArtifact.kind}`}>
                  {selectedArtifact.kind}
                </span>
              ) : null}
              {selectedArtifact ? (
                <span className="status-pill">{selectedArtifact.renderMode}</span>
              ) : null}
              {selectedNode?.path ? (
                <span className="artifact-path-label">{selectedNode.path}</span>
              ) : null}
            </div>

            <div
              className={`artifact-preview-stage${
                previewMode === "markdown" || previewMode === "html" ? " is-embedded" : ""
              }`}
            >
              {previewMode === "markdown" && sessionId && selectedPath ? (
                <iframe
                  className="workspace-frame"
                  src={buildSessionWorkspaceRenderUrl(sessionId, selectedPath)}
                  title={selectedPath}
                />
              ) : null}

              {previewMode === "html" && sessionId && selectedPath ? (
                <iframe
                  className="workspace-frame"
                  src={buildSessionWorkspaceRawUrl(sessionId, selectedPath)}
                  title={selectedPath}
                />
              ) : null}

              {previewMode === "text" ? (
                previewError ? (
                  <p className="error-copy">{previewError}</p>
                ) : (
                  <pre className="workspace-text-preview">{textPreview}</pre>
                )
              ) : null}

              {previewMode === "directory" ? (
                <div className="empty-state">
                  Open a file from the tree to preview it here. Markdown renders
                  inline, HTML opens in a frame, and JSON or text shows as code.
                </div>
              ) : null}

              {previewMode === "unsupported" ? (
                <div className="empty-state">
                  This file exists in the workspace, but there is no inline
                  renderer for its type yet.
                </div>
              ) : null}

              {previewMode === "empty" ? (
                <div className="empty-state">
                  Select an artifact from the left to inspect what the runtime
                  wrote into this session workspace.
                </div>
              ) : null}
            </div>
          </div>
        </section>

        <aside className="artifact-rail">
          <div className="artifact-rail-head">
            <div>
              <p className="eyebrow">Artifact monitor</p>
              <h2>Selection details</h2>
            </div>
            <span className="status-pill tone-sun">{previewLabel}</span>
          </div>

          <div className="rail-summary">
            <div className="rail-summary-item">
              <span>Kind</span>
              <strong>
                {selectedArtifact
                  ? humanizeLabel(selectedArtifact.kind)
                  : selectedNode
                    ? humanizeLabel(selectedNode.kind)
                    : "None"}
              </strong>
            </div>
            <div className="rail-summary-item">
              <span>Size</span>
              <strong>
                {selectedArtifact ? formatArtifactSize(selectedArtifact.sizeBytes) : "n/a"}
              </strong>
            </div>
            <div className="rail-summary-item">
              <span>Task</span>
              <strong>
                {selectedArtifact
                  ? compactId(selectedArtifact.taskId)
                  : selectedTask
                    ? compactId(selectedTask.id)
                    : "All"}
              </strong>
            </div>
            <div className="rail-summary-item">
              <span>Run</span>
              <strong>{selectedArtifact ? compactId(selectedArtifact.runId) : "n/a"}</strong>
            </div>
          </div>

          <section className="rail-section">
            <div className="rail-section-head">
              <div>
                <p className="eyebrow">Path</p>
                <h3>{selectedNode?.name ?? selectedArtifact?.displayName ?? "No selection"}</h3>
              </div>
            </div>
            <div className="artifact-rail-list">
              <div className="artifact-rail-row">
                <span>Workspace path</span>
                <strong>{selectedNode?.path ?? selectedArtifact?.path ?? "Not selected"}</strong>
              </div>
              <div className="artifact-rail-row">
                <span>Updated</span>
                <strong>
                  {selectedArtifact ? formatDateTime(selectedArtifact.updatedAt) : "Waiting"}
                </strong>
              </div>
              <div className="artifact-rail-row">
                <span>Media type</span>
                <strong>{selectedArtifact?.mediaType ?? "workspace file"}</strong>
              </div>
            </div>
          </section>

          <section className="rail-section">
            <div className="rail-section-head">
              <div>
                <p className="eyebrow">Lineage</p>
                <h3>Producer trail</h3>
              </div>
            </div>
            <p className="rail-copy">
              {selectedArtifact
                ? excerpt(selectedArtifact.description, 140)
                : "Artifact metadata will appear here when a generated output is selected."}
            </p>
            <div className="artifact-rail-list">
              <div className="artifact-rail-row">
                <span>Producer</span>
                <strong>
                  {selectedArtifact?.producerKind
                    ? selectedArtifact.producerRefId
                      ? `${selectedArtifact.producerKind} ${compactId(selectedArtifact.producerRefId)}`
                      : selectedArtifact.producerKind
                    : "Not attached"}
                </strong>
              </div>
            </div>
          </section>

          <section className="rail-section">
            <div className="rail-section-head">
              <div>
                <p className="eyebrow">Quick actions</p>
                <h3>Open routes</h3>
              </div>
            </div>
            <div className="artifact-actions">
              {renderedHref ? (
                <a className="ghost-button" href={renderedHref} rel="noreferrer" target="_blank">
                  Open render
                </a>
              ) : null}
              {rawHref ? (
                <a className="ghost-button" href={rawHref} rel="noreferrer" target="_blank">
                  Open raw file
                </a>
              ) : null}
              {sessionId ? (
                <Link className="ghost-button" href={`/sessions/${sessionId}/execution`}>
                  View execution
                </Link>
              ) : null}
              {sessionId ? (
                <Link className="ghost-button" href={`/sessions/${sessionId}/chat`}>
                  Back to chat
                </Link>
              ) : null}
            </div>
          </section>
        </aside>
      </section>
    </div>
  );
}

function WorkspaceTreeNode({
  node,
  selectedPath,
  onSelectPath,
  depth = 0,
}: {
  node: WorkspaceNode;
  selectedPath: string | null;
  onSelectPath: (path: string) => void;
  depth?: number;
}) {
  return (
    <div className="workspace-tree-node">
      <button
        className={`workspace-node-button${
          selectedPath === node.path ? " is-active" : ""
        }`}
        onClick={() => {
          if (node.kind === "file") {
            onSelectPath(node.path);
          }
        }}
        style={{ paddingLeft: `${14 + depth * 16}px` }}
        type="button"
      >
        <small>{node.kind === "directory" ? "dir" : "file"}</small>
        <strong>{node.name}</strong>
      </button>
      {node.children.map((child) => (
        <WorkspaceTreeNode
          depth={depth + 1}
          key={child.path || child.name}
          node={child}
          onSelectPath={onSelectPath}
          selectedPath={selectedPath}
        />
      ))}
    </div>
  );
}

function findWorkspaceNode(node: WorkspaceNode, path: string): WorkspaceNode | null {
  if (node.path === path) {
    return node;
  }

  for (const child of node.children) {
    const match = findWorkspaceNode(child, path);
    if (match) {
      return match;
    }
  }

  return null;
}

function countWorkspaceFiles(node: WorkspaceNode | null): number {
  if (!node) {
    return 0;
  }

  return node.kind === "file"
    ? 1
    : node.children.reduce((total, child) => total + countWorkspaceFiles(child), 0);
}

function getPreviewMode(
  node: WorkspaceNode | null,
  artifact: ArtifactRecord | null,
): PreviewMode {
  if (!node) {
    return "empty";
  }
  if (node.kind === "directory") {
    return "directory";
  }

  if (artifact) {
    if (artifact.renderMode === "markdown") {
      return "markdown";
    }
    if (artifact.renderMode === "html") {
      return "html";
    }
    if (artifact.renderMode === "json" || artifact.renderMode === "text") {
      return "text";
    }
  }

  const extension = node.path.split(".").pop()?.toLowerCase();
  if (extension === "md" || extension === "markdown") {
    return "markdown";
  }
  if (extension === "html" || extension === "htm") {
    return "html";
  }
  if (
    extension &&
    ["txt", "json", "rs", "ts", "tsx", "js", "css", "toml", "yml", "yaml"].includes(
      extension,
    )
  ) {
    return "text";
  }

  return "unsupported";
}

function describePreviewMode(previewMode: PreviewMode): string {
  switch (previewMode) {
    case "markdown":
      return "Rendered";
    case "html":
      return "Live HTML";
    case "text":
      return "Code view";
    case "directory":
      return "Folder";
    case "unsupported":
      return "Unsupported";
    case "empty":
      return "Waiting";
  }
}

export function pickDefaultWorkspacePath(tree: WorkspaceNode | null): string | null {
  if (!tree) {
    return null;
  }

  const preferred = findFirstFile(tree, (node) => node.path.endsWith("report.html"));
  if (preferred) {
    return preferred.path;
  }

  const markdown = findFirstFile(tree, (node) => node.path.endsWith(".md"));
  if (markdown) {
    return markdown.path;
  }

  return findFirstFile(tree, () => true)?.path ?? null;
}

function findFirstFile(
  node: WorkspaceNode,
  predicate: (node: WorkspaceNode) => boolean,
): WorkspaceNode | null {
  if (node.kind === "file" && predicate(node)) {
    return node;
  }

  for (const child of node.children) {
    const match = findFirstFile(child, predicate);
    if (match) {
      return match;
    }
  }

  return null;
}
