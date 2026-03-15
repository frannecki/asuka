"use client";

import { useEffect, useState } from "react";

import { getTaskExecution, listTasks } from "@/lib/api";
import { HarnessPanel } from "@/components/harness-panel";
import type {
  PlanDetail,
  RunStepRecord,
  TaskExecutionDetail,
  TaskRecord,
  ToolInvocationRecord,
} from "@/lib/types";

type SessionExecutionViewProps = {
  sessionId: string;
};

export function SessionExecutionView({ sessionId }: SessionExecutionViewProps) {
  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [executionDetail, setExecutionDetail] = useState<TaskExecutionDetail | null>(null);
  const [planDetail, setPlanDetail] = useState<PlanDetail | null>(null);
  const [runSteps, setRunSteps] = useState<RunStepRecord[]>([]);
  const [toolInvocations, setToolInvocations] = useState<ToolInvocationRecord[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void listTasks(sessionId)
      .then(async (nextTasks) => {
        if (cancelled) {
          return;
        }

        setTasks(nextTasks);
        const firstTask = nextTasks[0] ?? null;
        setActiveTaskId(firstTask?.id ?? null);
        if (!firstTask) {
          setExecutionDetail(null);
          setPlanDetail(null);
          setRunSteps([]);
          setToolInvocations([]);
          return;
        }

        const detail = await getTaskExecution(firstTask.id);
        if (cancelled) {
          return;
        }

        const group = detail.timelineGroups[0] ?? null;
        setExecutionDetail(detail);
        setPlanDetail(detail.planDetail);
        setRunSteps(group?.runSteps ?? []);
        setToolInvocations(group?.toolInvocations ?? []);
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(
            loadError instanceof Error
              ? loadError.message
              : "Failed to load execution data.",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  async function handleSelectTask(taskId: string) {
    setActiveTaskId(taskId);
    try {
      const detail = await getTaskExecution(taskId);
      const group =
        detail.timelineGroups.find((candidate) => candidate.run.id === detail.task.latestRunId) ??
        detail.timelineGroups[0] ??
        null;
      setExecutionDetail(detail);
      setPlanDetail(detail.planDetail);
      setRunSteps(group?.runSteps ?? []);
      setToolInvocations(group?.toolInvocations ?? []);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Failed to switch execution context.",
      );
    }
  }

  return (
    <div className="stack-gap">
      {error ? <p className="error-copy">{error}</p> : null}
      <HarnessPanel
        activeTaskId={activeTaskId}
        activity={[]}
        executionDetail={executionDetail}
        modelLabel={null}
        onSelectTaskId={(taskId) => void handleSelectTask(taskId)}
        planDetail={planDetail}
        runSteps={runSteps}
        status="idle"
        tasks={tasks}
        toolInvocations={toolInvocations}
      />
    </div>
  );
}
