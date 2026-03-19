"use client";

import { describeRunEvent } from "@/components/chat-activity";
import type {
  LineageEdgeRecord,
  LineageNodeKind,
  LineageNodeRecord,
  PlanDetail,
  RunEventEnvelope,
  RunStepRecord,
  TaskRecord,
  TaskExecutionDetail,
  ToolInvocationRecord,
} from "@/lib/types";
import {
  compactId,
  excerpt,
  formatDateTime,
  humanizeLabel,
} from "@/lib/view";

type HarnessPanelProps = {
  tasks: TaskRecord[];
  activeTaskId: string | null;
  onSelectTaskId: (taskId: string) => void;
  executionDetail: TaskExecutionDetail | null;
  planDetail: PlanDetail | null;
  runSteps: RunStepRecord[];
  toolInvocations: ToolInvocationRecord[];
  activity: RunEventEnvelope[];
  modelLabel: string | null;
  status: string;
};

export function HarnessPanel({
  tasks,
  activeTaskId,
  onSelectTaskId,
  executionDetail,
  planDetail,
  runSteps,
  toolInvocations,
  activity,
  modelLabel,
  status,
}: HarnessPanelProps) {
  const selectedTask =
    tasks.find((task) => task.id === activeTaskId) ?? tasks[0] ?? null;
  const recentActivity = activity.slice(-8).reverse();

  return (
    <section className="panel stack-gap">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Execution storyboard</p>
          <h2>Plans, runs, steps, tools, lineage, and artifacts in one view</h2>
        </div>
        <div className="stack-inline">
          {modelLabel ? <span className="status-pill tone-sky">{modelLabel}</span> : null}
          <span className="status-pill tone-sun">{humanizeLabel(status)}</span>
        </div>
      </div>

      {tasks.length > 0 ? (
        <div className="task-selector">
          {tasks.map((task) => (
            <button
              className={`task-chip${task.id === selectedTask?.id ? " is-active" : ""}`}
              key={task.id}
              onClick={() => onSelectTaskId(task.id)}
              type="button"
            >
              <strong>{excerpt(task.title, 46)}</strong>
              <span>{humanizeLabel(task.status)}</span>
            </button>
          ))}
        </div>
      ) : (
        <div className="empty-state small">
          No durable harness tasks yet. Post a message to generate a task,
          plan, and persisted run steps.
        </div>
      )}

      {selectedTask ? (
        <>
          <article className="harness-summary-card">
            <div className="activity-topline">
              <span className="activity-badge">Task</span>
              <span>{formatDateTime(selectedTask.updatedAt)}</span>
            </div>
            <div className="activity-copy">
              <strong>{selectedTask.title}</strong>
              <p>{excerpt(selectedTask.summary || selectedTask.goal, 220)}</p>
            </div>
            <div className="status-strip">
              <span className="status-pill tone-mint">{humanizeLabel(selectedTask.status)}</span>
              {selectedTask.latestRunId ? (
                <span className="status-pill">Run {compactId(selectedTask.latestRunId)}</span>
              ) : null}
            </div>
          </article>

          <div className="harness-shell">
            <div className="harness-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Artifact groups</p>
                  <h3>Output bundles by run</h3>
                </div>
              </div>
              <div className="artifact-group-list">
                {executionDetail?.artifactGroups.map((group) => (
                  <article className="artifact-group-card" key={group.id}>
                    <div className="activity-topline">
                      <span className="activity-badge">Run {compactId(group.runId)}</span>
                      <span>{formatDateTime(group.createdAt)}</span>
                    </div>
                    <div className="activity-copy">
                      <strong>{group.title}</strong>
                      <p>{excerpt(group.summary, 120)}</p>
                    </div>
                    <div className="status-strip">
                      <span className="status-pill">{group.artifactIds.length} artifacts</span>
                      {group.primaryArtifactId ? (
                        <span className="status-pill">
                          Primary {compactId(group.primaryArtifactId)}
                        </span>
                      ) : null}
                    </div>
                  </article>
                ))}
                {executionDetail?.artifactGroups.length ? null : (
                  <div className="empty-state small">
                    Artifact groups appear when a run emits durable workspace outputs.
                  </div>
                )}
              </div>
            </div>

            <div className="harness-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Run timeline</p>
                  <h3>Grouped run history</h3>
                </div>
              </div>
              <div className="timeline-group-list">
                {executionDetail?.timelineGroups.map((group) => (
                  <article className="timeline-group-card" key={group.id}>
                    <div className="activity-topline">
                      <span className="activity-badge">Run {compactId(group.run.id)}</span>
                      <span>{humanizeLabel(group.run.status)}</span>
                    </div>
                    <div className="activity-copy">
                      <strong>
                        {group.run.selectedProvider ?? "Local runtime"} ·{" "}
                        {group.run.selectedModel ?? "fallback"}
                      </strong>
                      <p>
                        {group.runSteps.length} step(s), {group.toolInvocations.length} tool call(s),{" "}
                        {group.artifacts.length} artifact(s)
                      </p>
                    </div>
                    <div className="timeline-chip-row">
                      {group.runSteps.map((step) => (
                        <span className="timeline-chip" key={step.id}>
                          #{step.sequence} {step.title}
                        </span>
                      ))}
                    </div>
                    <div className="timeline-chip-row">
                      {group.artifacts.map((artifact) => (
                        <span className="timeline-chip artifact" key={artifact.id}>
                          {artifact.displayName}
                        </span>
                      ))}
                    </div>
                  </article>
                ))}
                {executionDetail?.timelineGroups.length ? null : (
                  <div className="empty-state small">
                    Grouped run history will appear here once the selected task
                    has durable execution data.
                  </div>
                )}
              </div>
            </div>

            <div className="harness-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Plan</p>
                  <h3>Execution steps</h3>
                </div>
              </div>
              <div className="plan-step-list">
                {planDetail?.steps.map((step) => (
                  <article className="plan-step-card" key={step.id}>
                    <div className="activity-topline">
                      <span className="activity-badge">Step {step.ordinal}</span>
                      <span>{humanizeLabel(step.status)}</span>
                    </div>
                    <div className="activity-copy">
                      <strong>{step.title}</strong>
                      <p>{excerpt(step.description, 140)}</p>
                    </div>
                  </article>
                ))}
                {planDetail?.steps.length ? null : (
                  <div className="empty-state small">
                    No plan has been loaded for the selected task.
                  </div>
                )}
              </div>
            </div>

            <div className="harness-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Lineage</p>
                  <h3>Execution graph</h3>
                </div>
              </div>
              {executionDetail?.lineageEdges.length ? (
                <ExecutionGraph
                  edges={executionDetail.lineageEdges}
                  nodes={executionDetail.lineageNodes}
                />
              ) : (
                <div className="lineage-list">
                  <div className="empty-state small">
                    Execution graph nodes will appear here once runs produce
                    artifacts or tool activity.
                  </div>
                </div>
              )}
            </div>

            <div className="harness-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Durable steps</p>
                  <h3>Persisted step history</h3>
                </div>
              </div>
              <div className="run-step-list">
                {runSteps.map((step) => (
                  <article className="run-step-card" key={step.id}>
                    <div className="activity-topline">
                      <span className="activity-badge">#{step.sequence}</span>
                      <span>{humanizeLabel(step.status)}</span>
                    </div>
                    <div className="activity-copy">
                      <strong>{step.title}</strong>
                      <p>{excerpt(step.inputSummary, 160)}</p>
                    </div>
                    {step.outputSummary ? (
                      <details>
                        <summary>Output summary</summary>
                        <pre>{step.outputSummary}</pre>
                      </details>
                    ) : null}
                    {step.error ? <p className="error-copy">{step.error}</p> : null}
                  </article>
                ))}
                {runSteps.length === 0 ? (
                  <div className="empty-state small">
                    Run steps will appear here once the selected task has a run.
                  </div>
                ) : null}
              </div>
            </div>

            <div className="harness-section">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Tools</p>
                  <h3>Tool invocations</h3>
                </div>
              </div>
              <div className="tool-invocation-list">
                {toolInvocations.map((invocation) => (
                  <article className="tool-invocation-card" key={invocation.id}>
                    <div className="activity-topline">
                      <span className="activity-badge">{invocation.toolName}</span>
                      <span>{invocation.ok ? "ok" : "error"}</span>
                    </div>
                    <p>{humanizeLabel(invocation.toolSource)}</p>
                    <details>
                      <summary>Arguments</summary>
                      <pre>{JSON.stringify(invocation.argumentsJson, null, 2)}</pre>
                    </details>
                    <details>
                      <summary>Result</summary>
                      <pre>{JSON.stringify(invocation.resultJson, null, 2)}</pre>
                    </details>
                  </article>
                ))}
                {toolInvocations.length === 0 ? (
                  <div className="empty-state small">
                    No persisted tool invocations for the selected run yet.
                  </div>
                ) : null}
              </div>
            </div>
          </div>
        </>
      ) : null}

      <div className="harness-section">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Live stream</p>
            <h3>Recent streaming activity</h3>
          </div>
        </div>
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
                  <span>{formatDateTime(event.timestamp)}</span>
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
              Streamed lifecycle events still appear here while a run is active.
            </div>
          ) : null}
        </div>
      </div>
    </section>
  );
}

type GraphLayoutNode = {
  node: LineageNodeRecord;
  x: number;
  y: number;
  width: number;
  height: number;
};

function ExecutionGraph({
  nodes,
  edges,
}: {
  nodes: LineageNodeRecord[];
  edges: LineageEdgeRecord[];
}) {
  const layout = buildGraphLayout(nodes);
  const viewBoxHeight = Math.max(layout.height, 220);

  return (
    <div className="execution-graph-shell">
      <svg
        className="execution-graph"
        preserveAspectRatio="xMinYMin meet"
        viewBox={`0 0 ${layout.width} ${viewBoxHeight}`}
      >
        {edges.map((edge) => {
          const from = layout.nodeMap.get(edge.from);
          const to = layout.nodeMap.get(edge.to);
          if (!from || !to) {
            return null;
          }

          const startX = from.x + from.width;
          const startY = from.y + from.height / 2;
          const endX = to.x;
          const endY = to.y + to.height / 2;
          const midX = (startX + endX) / 2;
          const path = `M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${endY}, ${endX} ${endY}`;

          return (
            <g key={`${edge.from}-${edge.to}-${edge.relation}`}>
              <path className="graph-edge" d={path} />
              <text
                className="graph-edge-label"
                textAnchor="middle"
                x={midX}
                y={(startY + endY) / 2 - 6}
              >
                {edge.relation}
              </text>
            </g>
          );
        })}

        {layout.columns.map((column) => (
          <g key={column.key}>
            <text className="graph-column-label" x={column.x} y={24}>
              {column.label}
            </text>
          </g>
        ))}

        {layout.nodes.map((entry) => (
          <g className={`graph-node kind-${entry.node.kind}`} key={entry.node.id}>
            <rect
              className="graph-node-box"
              height={entry.height}
              rx={18}
              width={entry.width}
              x={entry.x}
              y={entry.y}
            />
            <text className="graph-node-title" x={entry.x + 14} y={entry.y + 24}>
              {truncate(entry.node.label, 30)}
            </text>
            <text className="graph-node-meta" x={entry.x + 14} y={entry.y + 44}>
              {entry.node.kind}
            </text>
            {entry.node.status ? (
              <text className="graph-node-status" x={entry.x + 14} y={entry.y + 62}>
                {entry.node.status}
              </text>
            ) : null}
          </g>
        ))}
      </svg>
    </div>
  );
}

function buildGraphLayout(nodes: LineageNodeRecord[]) {
  const columnConfig: Array<{ key: string; label: string; kinds: LineageNodeKind[] }> = [
    { key: "task", label: "Task", kinds: ["task"] },
    { key: "run", label: "Runs", kinds: ["run"] },
    { key: "activity", label: "Steps and tools", kinds: ["runStep", "toolInvocation"] },
    { key: "artifact", label: "Artifacts", kinds: ["artifact"] },
  ];
  const columnWidth = 220;
  const columnGap = 72;
  const rowHeight = 88;
  const rowGap = 18;
  const topPadding = 42;
  const leftPadding = 12;

  const positionedNodes: GraphLayoutNode[] = [];
  const nodeMap = new Map<string, GraphLayoutNode>();
  const columns = columnConfig.map((column, index) => ({
    ...column,
    x: leftPadding + index * (columnWidth + columnGap),
  }));

  for (const column of columns) {
    const columnNodes = nodes
      .filter((node) => column.kinds.includes(node.kind))
      .sort((left, right) => left.label.localeCompare(right.label));

    columnNodes.forEach((node, index) => {
      const entry = {
        node,
        x: column.x,
        y: topPadding + index * (rowHeight + rowGap),
        width: columnWidth,
        height: rowHeight,
      };
      positionedNodes.push(entry);
      nodeMap.set(node.id, entry);
    });
  }

  const maxRows = Math.max(
    1,
    ...columns.map((column) =>
      nodes.filter((node) => column.kinds.includes(node.kind)).length,
    ),
  );

  return {
    width: leftPadding * 2 + columns.length * columnWidth + (columns.length - 1) * columnGap,
    height: topPadding + maxRows * rowHeight + Math.max(0, maxRows - 1) * rowGap + 18,
    columns,
    nodes: positionedNodes,
    nodeMap,
  };
}

function truncate(value: string, max: number): string {
  return value.length > max ? `${value.slice(0, max - 1)}…` : value;
}
