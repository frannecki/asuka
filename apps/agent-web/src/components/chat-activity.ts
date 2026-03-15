import type { RunEventEnvelope } from "../lib/types";

export type ChatActivityDescriptor = {
  badge: string;
  title: string;
  summary: string;
  detail: string | null;
  tone: "neutral" | "accent" | "warning" | "success";
};

function asString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function formatJson(value: unknown): string | null {
  if (value == null) {
    return null;
  }

  return JSON.stringify(value, null, 2);
}

function formatStepLabel(stepType: string | null): string {
  if (!stepType) {
    return "Run step";
  }

  return stepType
    .split(/[-.]/g)
    .map((segment) =>
      segment ? `${segment[0].toUpperCase()}${segment.slice(1)}` : segment,
    )
    .join(" ");
}

export function formatActivityPayload(payload: Record<string, unknown>): string {
  return JSON.stringify(payload, null, 2);
}

export function describeRunEvent(
  event: RunEventEnvelope,
): ChatActivityDescriptor {
  switch (event.eventType) {
    case "run.stream.ready":
      return {
        badge: "stream",
        title: "Stream connected",
        summary: "Inspector subscribed to live run events.",
        detail: null,
        tone: "accent",
      };
    case "run.started":
      return {
        badge: "run",
        title: "Run started",
        summary: "The agent loop is now executing.",
        detail: null,
        tone: "accent",
      };
    case "model.selected": {
      const providerName = asString(event.payload.providerName) ?? "Unknown provider";
      const modelName = asString(event.payload.modelName) ?? "unknown-model";
      return {
        badge: "model",
        title: "Model selected",
        summary: `${providerName} · ${modelName}`,
        detail: null,
        tone: "accent",
      };
    }
    case "run.step.started": {
      const stepType = asString(event.payload.stepType);
      return {
        badge: "step",
        title: formatStepLabel(stepType),
        summary:
          asString(event.payload.message) ?? "The runtime advanced to another step.",
        detail: null,
        tone: stepType === "model-fallback" || stepType === "memory-write-skipped"
          ? "warning"
          : "neutral",
      };
    }
    case "tool.call.started":
      return {
        badge: "tool",
        title: "Tool call started",
        summary: asString(event.payload.toolName) ?? "Unknown tool",
        detail: formatJson(event.payload.arguments),
        tone: "neutral",
      };
    case "tool.call.completed":
      return {
        badge: "tool",
        title: "Tool call completed",
        summary: asString(event.payload.toolName) ?? "Unknown tool",
        detail: formatJson(event.payload.result),
        tone: "success",
      };
    case "subagent.started":
      return {
        badge: "subagent",
        title: "Subagent started",
        summary: asString(event.payload.subagent) ?? "Unnamed subagent",
        detail: asString(event.payload.task),
        tone: "neutral",
      };
    case "subagent.completed":
      return {
        badge: "subagent",
        title: "Subagent completed",
        summary: asString(event.payload.subagent) ?? "Unnamed subagent",
        detail: asString(event.payload.summary),
        tone: "success",
      };
    case "memory.retrieved": {
      const hits = Array.isArray(event.payload.hits) ? event.payload.hits : [];
      const detail = hits.length
        ? hits
            .map((hit) => {
              if (!hit || typeof hit !== "object") {
                return "Unknown memory hit";
              }

              const title =
                asString((hit as Record<string, unknown>).documentTitle) ?? "Untitled";
              const namespace =
                asString((hit as Record<string, unknown>).namespace) ?? "global";
              const memoryScope =
                asString((hit as Record<string, unknown>).memoryScope) ?? "global";
              return `${title} [${memoryScope}/${namespace}]`;
            })
            .join("\n")
        : null;

      return {
        badge: "memory",
        title: "Memory retrieved",
        summary: `${hits.length} relevant chunk${hits.length === 1 ? "" : "s"} found`,
        detail,
        tone: "neutral",
      };
    }
    case "memory.written": {
      const title = asString(event.payload.title) ?? "Untitled memory";
      const namespace = asString(event.payload.namespace) ?? "global";
      const memoryScope = asString(event.payload.memoryScope) ?? "global";
      return {
        badge: "memory",
        title: "Memory saved",
        summary: `${title} [${memoryScope}/${namespace}]`,
        detail: asString(event.payload.documentId),
        tone: "success",
      };
    }
    case "message.delta":
      return {
        badge: "stream",
        title: "Streaming reply",
        summary: asString(event.payload.delta) ?? "Partial assistant response",
        detail: null,
        tone: "neutral",
      };
    case "run.completed":
      return {
        badge: "run",
        title: "Run completed",
        summary: "The assistant reply has been committed to the session.",
        detail: asString(event.payload.messageId),
        tone: "success",
      };
    case "run.failed":
      return {
        badge: "run",
        title: "Run failed",
        summary: asString(event.payload.message) ?? "The run terminated with an error.",
        detail: null,
        tone: "warning",
      };
    default:
      return {
        badge: "event",
        title: event.eventType,
        summary: "Additional runtime activity was recorded.",
        detail: formatActivityPayload(event.payload),
        tone: "neutral",
      };
  }
}
