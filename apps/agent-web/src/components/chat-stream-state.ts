import type { RunEventEnvelope } from "../lib/types";

export type ChatStreamState = {
  activity: RunEventEnvelope[];
  activeRunId: string | null;
  draftReply: string;
  lastSequence: number;
  modelLabel: string | null;
  status: string;
};

export type ChatStreamTransition = ChatStreamState & {
  shouldReconnect: boolean;
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
    activeRunId: null,
    draftReply: "",
    lastSequence: 0,
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
  if (envelope.sequence <= current.lastSequence) {
    return {
      ...current,
      sessionToReload: null,
      shouldCloseStream: false,
      shouldReconnect: false,
      shouldRefreshSessions: false,
    };
  }

  const next: ChatStreamTransition = {
    activity: [envelope, ...current.activity].slice(0, MAX_ACTIVITY_ITEMS),
    activeRunId: current.activeRunId,
    draftReply: current.draftReply,
    lastSequence: envelope.sequence,
    modelLabel: current.modelLabel,
    status: current.status,
    sessionToReload: null,
    shouldCloseStream: false,
    shouldReconnect: false,
    shouldRefreshSessions: false,
  };

  if (eventName === "run.stream.ready") {
    next.activeRunId = envelope.runId;
    if (
      current.status === "idle" ||
      current.status === "recovering" ||
      current.status === "stream-disconnected"
    ) {
      next.status = "running";
    }
  }

  if (eventName === "message.delta") {
    const delta =
      typeof envelope.payload.delta === "string" ? envelope.payload.delta : "";
    next.draftReply = next.draftReply
      ? `${next.draftReply} ${delta}`.trim()
      : delta;
  }

  if (eventName === "run.started") {
    next.activeRunId = envelope.runId;
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
    next.activeRunId = null;
    next.status = "completed";
    next.draftReply = "";
    next.sessionToReload = envelope.sessionId;
    next.shouldCloseStream = true;
    next.shouldRefreshSessions = true;
  }

  if (eventName === "run.failed") {
    next.activeRunId = null;
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
    shouldReconnect: current.activeRunId !== null,
    shouldRefreshSessions: false,
    status: "stream-disconnected",
  };
}

export function replayRunHistory(
  current: ChatStreamState,
  events: RunEventEnvelope[],
): ChatStreamState {
  return events.reduce<ChatStreamState>((state, event) => {
    const transition = applyRunStreamEvent(state, event.eventType, event);
    return {
      activity: transition.activity,
      activeRunId: transition.activeRunId,
      draftReply: transition.draftReply,
      lastSequence: transition.lastSequence,
      modelLabel: transition.modelLabel,
      status: transition.status,
    };
  }, current);
}

export function formatActivityPayload(payload: Record<string, unknown>): string {
  return JSON.stringify(payload, null, 2);
}
