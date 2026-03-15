"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

import {
  buildSessionWorkspaceRawUrl,
  buildSessionWorkspaceRenderUrl,
} from "@/lib/api";
import type { ArtifactRecord, WorkspaceNode } from "@/lib/types";

type WorkspacePanelProps = {
  artifacts: ArtifactRecord[];
  sessionId: string | null;
  tree: WorkspaceNode | null;
  selectedPath: string | null;
  onSelectPath: (path: string) => void;
};

type PreviewMode = "directory" | "markdown" | "html" | "text" | "unsupported" | "empty";

export function WorkspacePanel({
  artifacts,
  sessionId,
  tree,
  selectedPath,
  onSelectPath,
}: WorkspacePanelProps) {
  const [textPreview, setTextPreview] = useState<string>("");
  const [previewError, setPreviewError] = useState<string | null>(null);

  const selectedNode = useMemo(
    () => (tree && selectedPath ? findWorkspaceNode(tree, selectedPath) : null),
    [tree, selectedPath],
  );
  const selectedArtifact = useMemo(
    () => artifacts.find((artifact) => artifact.path === selectedPath) ?? null,
    [artifacts, selectedPath],
  );
  const previewMode = getPreviewMode(selectedNode, selectedArtifact);

  useEffect(() => {
    if (!sessionId || !selectedPath || previewMode !== "text") {
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
  }, [previewMode, selectedPath, sessionId]);

  return (
    <section className="panel stack-gap">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Workspace</p>
          <h2>Session artifacts</h2>
        </div>
      </div>

      <div className="workspace-shell">
        <div className="workspace-tree">
          <div className="workspace-artifact-list">
            {artifacts.length > 0 ? (
              artifacts.map((artifact) => (
                <button
                  className={`artifact-card${
                    selectedPath === artifact.path ? " is-active" : ""
                  }`}
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
                  <footer>
                    <span>{artifact.path}</span>
                    <span>{formatArtifactSize(artifact.sizeBytes)}</span>
                  </footer>
                </button>
              ))
            ) : (
              <div className="empty-state small">
                No durable artifact records yet. Completed runs will register
                report, markdown, and JSON outputs here.
              </div>
            )}
          </div>

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
              No session workspace files yet. Completed runs will emit markdown,
              JSON, and HTML artifacts here.
            </div>
          )}
        </div>

        <div className="workspace-preview">
          <header className="workspace-preview-header">
            <strong>{selectedNode?.name ?? "No file selected"}</strong>
            {selectedNode?.path ? <span>{selectedNode.path}</span> : null}
          </header>
          {selectedArtifact ? (
            <div className="workspace-preview-meta">
              <span>{selectedArtifact.displayName}</span>
              <span>{selectedArtifact.kind}</span>
              <span>{formatArtifactSize(selectedArtifact.sizeBytes)}</span>
            </div>
          ) : null}

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
            <div className="empty-state small">
              Select a file to preview it here. Markdown opens as rendered
              content, HTML opens in an iframe, and JSON/text opens inline.
            </div>
          ) : null}

          {previewMode === "unsupported" ? (
            <div className="empty-state small">
              This file type is available in the tree but does not have an
              inline preview yet.
            </div>
          ) : null}

          {previewMode === "empty" ? (
            <div className="empty-state small">
              Select a generated artifact to inspect the session workspace.
            </div>
          ) : null}
        </div>

        <div className="workspace-meta">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Metadata</p>
              <h3>{selectedArtifact?.displayName ?? "Artifact details"}</h3>
            </div>
          </div>

          {selectedArtifact ? (
            <div className="workspace-meta-list">
              <div className="workspace-meta-row">
                <span>Kind</span>
                <strong>{selectedArtifact.kind}</strong>
              </div>
              <div className="workspace-meta-row">
                <span>Render mode</span>
                <strong>{selectedArtifact.renderMode}</strong>
              </div>
              <div className="workspace-meta-row">
                <span>Media type</span>
                <strong>{selectedArtifact.mediaType}</strong>
              </div>
              <div className="workspace-meta-row">
                <span>Path</span>
                <strong>{selectedArtifact.path}</strong>
              </div>
              <div className="workspace-meta-row">
                <span>Size</span>
                <strong>{formatArtifactSize(selectedArtifact.sizeBytes)}</strong>
              </div>
              <div className="workspace-meta-row">
                <span>Task</span>
                <strong>{selectedArtifact.taskId.slice(0, 8)}</strong>
              </div>
              <div className="workspace-meta-row">
                <span>Run</span>
                <strong>{selectedArtifact.runId.slice(0, 8)}</strong>
              </div>
              <div className="workspace-meta-row">
                <span>Updated</span>
                <strong>{new Date(selectedArtifact.updatedAt).toLocaleString()}</strong>
              </div>

              <article className="workspace-lineage-card">
                <p className="eyebrow">Lineage</p>
                <p>{selectedArtifact.description}</p>
                {selectedArtifact.producerKind ? (
                  <p className="hint-copy">
                    Produced by {selectedArtifact.producerKind}{" "}
                    {selectedArtifact.producerRefId
                      ? selectedArtifact.producerRefId.slice(0, 8)
                      : ""}
                  </p>
                ) : (
                  <p className="hint-copy">
                    Producer metadata has not been attached to this artifact.
                  </p>
                )}
                {sessionId ? (
                  <div className="button-row">
                    <Link className="ghost-button" href={`/sessions/${sessionId}/execution`}>
                      View execution graph
                    </Link>
                    <Link className="ghost-button" href={`/sessions/${sessionId}/chat`}>
                      Back to chat
                    </Link>
                  </div>
                ) : null}
              </article>
            </div>
          ) : (
            <div className="empty-state small">
              Select an artifact to inspect its metadata, producer, and related
              workspace provenance.
            </div>
          )}
        </div>
      </div>
    </section>
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
        style={{ paddingLeft: `${12 + depth * 14}px` }}
        type="button"
      >
        <span>{node.kind === "directory" ? "dir" : "file"}</span>
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

function formatArtifactSize(sizeBytes: number): string {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }
  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
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
