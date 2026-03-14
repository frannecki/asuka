import type { RunEventEnvelope } from "../lib/types";

export type ChatStreamState = {
  activity: RunEventEnvelope[];
  draftReply: string;
  modelLabel: string | null;
  status: string;
};

export type ChatStreamTransition = ChatStreamState & {
  sessionToReload: string | null;
  shouldCloseStream: boolean;
  shouldRefreshSessions: boolean;
};

const MAX_ACTIVITY_ITEMS = 30;

export function createChatStreamState(
  overrides: Partial<ChatStreamState> = {},
): ChatStreamState {
  return {
    activity: [],
    draftReply: "",
    modelLabel: null,
    status: "idle",
    ...overrides,
  };
}

export function applyRunStreamEvent(
  current: ChatStreamState,
  eventName: string,
  envelope: RunEventEnvelope,
): ChatStreamTransition {
  const next: ChatStreamTransition = {
    activity: [envelope, ...current.activity].slice(0, MAX_ACTIVITY_ITEMS),
    draftReply: current.draftReply,
    modelLabel: current.modelLabel,
    status: current.status,
    sessionToReload: null,
    shouldCloseStream: false,
    shouldRefreshSessions: false,
  };

  if (eventName === "message.delta") {
    const delta =
      typeof envelope.payload.delta === "string" ? envelope.payload.delta : "";
    next.draftReply = next.draftReply
      ? `${next.draftReply} ${delta}`.trim()
      : delta;
  }

  if (eventName === "run.started") {
    next.status = "running";
  }

  if (eventName === "model.selected") {
    const providerName =
      typeof envelope.payload.providerName === "string"
        ? envelope.payload.providerName
        : "Unknown provider";
    const modelName =
      typeof envelope.payload.modelName === "string"
        ? envelope.payload.modelName
        : "unknown-model";
    next.modelLabel = `${providerName} · ${modelName}`;
  }

  if (eventName === "run.completed") {
    next.status = "completed";
    next.draftReply = "";
    next.sessionToReload = envelope.sessionId;
    next.shouldCloseStream = true;
    next.shouldRefreshSessions = true;
  }

  if (eventName === "run.failed") {
    next.status = "failed";
    next.shouldCloseStream = true;
  }

  return next;
}

export function disconnectRunStream(
  current: ChatStreamState,
): ChatStreamTransition {
  return {
    ...current,
    sessionToReload: null,
    shouldCloseStream: true,
    shouldRefreshSessions: false,
    status: "stream-disconnected",
  };
}

export function formatActivityPayload(payload: Record<string, unknown>): string {
  return JSON.stringify(payload, null, 2);
}
