"use client";

import { useEffect, useState } from "react";

import {
  getSessionWorkspaceTree,
  listSessionArtifacts,
  listTasks,
} from "@/lib/api";
import {
  pickDefaultWorkspacePath,
  WorkspacePanel,
} from "@/components/workspace-panel";
import type { ArtifactRecord, TaskRecord, WorkspaceNode } from "@/lib/types";

type SessionArtifactsViewProps = {
  sessionId: string;
};

export function SessionArtifactsView({ sessionId }: SessionArtifactsViewProps) {
  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [artifacts, setArtifacts] = useState<ArtifactRecord[]>([]);
  const [workspaceTree, setWorkspaceTree] = useState<WorkspaceNode | null>(null);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void Promise.all([
      listTasks(sessionId),
      listSessionArtifacts(sessionId),
      getSessionWorkspaceTree(sessionId),
    ])
      .then(([nextTasks, nextArtifacts, nextTree]) => {
        if (cancelled) {
          return;
        }

        const firstTask = nextTasks[0] ?? null;
        const visibleArtifacts = firstTask
          ? nextArtifacts.filter((artifact) => artifact.taskId === firstTask.id)
          : nextArtifacts;
        setTasks(nextTasks);
        setArtifacts(nextArtifacts);
        setWorkspaceTree(nextTree);
        setSelectedTaskId(firstTask?.id ?? null);
        setSelectedPath(
          visibleArtifacts[0]?.path ?? pickDefaultWorkspacePath(nextTree) ?? null,
        );
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(
            loadError instanceof Error
              ? loadError.message
              : "Failed to load session artifacts.",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  const filteredArtifacts = selectedTaskId
    ? artifacts.filter((artifact) => artifact.taskId === selectedTaskId)
    : artifacts;
  const visibleSelectedPath =
    selectedPath && filteredArtifacts.some((artifact) => artifact.path === selectedPath)
      ? selectedPath
      : filteredArtifacts[0]?.path ?? pickDefaultWorkspacePath(workspaceTree) ?? null;

  return (
    <div className="stack-gap">
      {tasks.length > 0 ? (
        <div className="task-selector">
          {tasks.map((task) => (
            <button
              className={`task-chip${task.id === selectedTaskId ? " is-active" : ""}`}
              key={task.id}
              onClick={() => setSelectedTaskId(task.id)}
              type="button"
            >
              <strong>{task.title}</strong>
              <span>{task.status}</span>
            </button>
          ))}
        </div>
      ) : null}
      {error ? <p className="error-copy">{error}</p> : null}
      <WorkspacePanel
        artifacts={filteredArtifacts}
        onSelectPath={setSelectedPath}
        selectedPath={visibleSelectedPath}
        sessionId={sessionId}
        tree={workspaceTree}
      />
    </div>
  );
}
