import assert from "node:assert/strict";
import test from "node:test";

import type { RunEventEnvelope } from "../lib/types";
import {
  applyRunStreamEvent,
  createChatStreamState,
  disconnectRunStream,
  formatActivityPayload,
} from "./chat-stream-state.ts";

function makeRunEvent(
  eventType: string,
  sequence: number,
  payload: Record<string, unknown> = {},
): RunEventEnvelope {
  return {
    eventType,
    runId: "run-1",
    sessionId: "session-1",
    timestamp: `2026-03-14T09:00:${String(sequence).padStart(2, "0")}Z`,
    sequence,
    payload,
  };
}

function stateFromTransition(
  transition: ReturnType<typeof applyRunStreamEvent> | ReturnType<typeof disconnectRunStream>,
) {
  return {
    activity: transition.activity,
    draftReply: transition.draftReply,
    modelLabel: transition.modelLabel,
    status: transition.status,
  };
}

test("applyRunStreamEvent tracks streamed activity and completion flags", () => {
  let state = createChatStreamState({ status: "running" });

  let transition = applyRunStreamEvent(
    state,
    "model.selected",
    makeRunEvent("model.selected", 1, {
      providerName: "OpenRouter",
      modelName: "demo-model",
    }),
  );
  assert.equal(transition.modelLabel, "OpenRouter · demo-model");
  assert.equal(transition.status, "running");
  assert.equal(transition.shouldCloseStream, false);
  state = stateFromTransition(transition);

  transition = applyRunStreamEvent(
    state,
    "tool.call.started",
    makeRunEvent("tool.call.started", 2, {
      toolName: "session.context.snapshot",
    }),
  );
  assert.equal(transition.activity[0]?.eventType, "tool.call.started");
  state = stateFromTransition(transition);

  transition = applyRunStreamEvent(
    state,
    "subagent.started",
    makeRunEvent("subagent.started", 3, {
      subagent: "research-analyst",
    }),
  );
  assert.equal(transition.activity[0]?.eventType, "subagent.started");
  state = stateFromTransition(transition);

  transition = applyRunStreamEvent(
    state,
    "message.delta",
    makeRunEvent("message.delta", 4, {
      delta: "Streaming provider reply",
    }),
  );
  assert.equal(transition.draftReply, "Streaming provider reply");
  state = stateFromTransition(transition);

  transition = applyRunStreamEvent(
    state,
    "run.completed",
    makeRunEvent("run.completed", 5, {
      status: "completed",
      messageId: "message-assistant-1",
    }),
  );
  assert.equal(transition.status, "completed");
  assert.equal(transition.draftReply, "");
  assert.equal(transition.shouldCloseStream, true);
  assert.equal(transition.shouldRefreshSessions, true);
  assert.equal(transition.sessionToReload, "session-1");
  assert.equal(transition.activity[0]?.eventType, "run.completed");
});

test("applyRunStreamEvent keeps model-fallback activity and caps inspector history", () => {
  let state = createChatStreamState({ status: "running" });

  const fallback = applyRunStreamEvent(
    state,
    "run.step.started",
    makeRunEvent("run.step.started", 1, {
      stepType: "model-fallback",
      message: "simulated upstream failure",
    }),
  );
  assert.equal(fallback.activity[0]?.eventType, "run.step.started");
  assert.match(
    formatActivityPayload(fallback.activity[0]?.payload ?? {}),
    /"stepType": "model-fallback"/,
  );
  assert.match(
    formatActivityPayload(fallback.activity[0]?.payload ?? {}),
    /"message": "simulated upstream failure"/,
  );

  state = stateFromTransition(fallback);
  for (let index = 2; index <= 35; index += 1) {
    state = stateFromTransition(
      applyRunStreamEvent(
        state,
        "tool.call.started",
        makeRunEvent("tool.call.started", index, {
          toolName: `tool-${index}`,
        }),
      ),
    );
  }

  assert.equal(state.activity.length, 30);
  assert.equal(state.activity[0]?.sequence, 35);
  assert.equal(state.activity.at(-1)?.sequence, 6);
});

test("disconnectRunStream marks the chat as disconnected and requests close", () => {
  const transition = disconnectRunStream(
    createChatStreamState({
      status: "running",
      draftReply: "Partial reply",
      modelLabel: "OpenRouter · demo-model",
      activity: [makeRunEvent("run.started", 1, { status: "running" })],
    }),
  );

  assert.equal(transition.status, "stream-disconnected");
  assert.equal(transition.draftReply, "Partial reply");
  assert.equal(transition.modelLabel, "OpenRouter · demo-model");
  assert.equal(transition.shouldCloseStream, true);
  assert.equal(transition.shouldRefreshSessions, false);
  assert.equal(transition.sessionToReload, null);
  assert.equal(transition.activity[0]?.eventType, "run.started");
});
