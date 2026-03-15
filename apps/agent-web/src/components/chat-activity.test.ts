import assert from "node:assert/strict";
import test from "node:test";

import type { RunEventEnvelope } from "../lib/types";
import {
  describeRunEvent,
  formatActivityPayload,
} from "./chat-activity.ts";

function makeRunEvent(
  eventType: string,
  payload: Record<string, unknown> = {},
): RunEventEnvelope {
  return {
    eventType,
    runId: "run-1",
    sessionId: "session-1",
    timestamp: "2026-03-14T09:00:00Z",
    sequence: 1,
    payload,
  };
}

test("describeRunEvent renders tool activity in a structured form", () => {
  const descriptor = describeRunEvent(
    makeRunEvent("tool.call.completed", {
      toolName: "session.context.snapshot",
      result: {
        providersSeen: 2,
        note: "Prototype tool call completed successfully.",
      },
    }),
  );

  assert.equal(descriptor.badge, "tool");
  assert.equal(descriptor.title, "Tool call completed");
  assert.equal(descriptor.summary, "session.context.snapshot");
  assert.equal(descriptor.tone, "success");
  assert.match(descriptor.detail ?? "", /Prototype tool call completed successfully/);
});

test("describeRunEvent renders model fallback and memory retrieval summaries", () => {
  const fallback = describeRunEvent(
    makeRunEvent("run.step.started", {
      stepType: "model-fallback",
      message: "simulated upstream failure",
    }),
  );
  assert.equal(fallback.title, "Model Fallback");
  assert.equal(fallback.tone, "warning");
  assert.equal(fallback.summary, "simulated upstream failure");

  const memory = describeRunEvent(
    makeRunEvent("memory.retrieved", {
      hits: [
        {
          documentTitle: "Platform Overview",
          namespace: "global",
          memoryScope: "global",
        },
        {
          documentTitle: "Provider Policy",
          namespace: "global",
          memoryScope: "project",
        },
      ],
    }),
  );
  assert.equal(memory.badge, "memory");
  assert.equal(memory.summary, "2 relevant chunks found");
  assert.match(memory.detail ?? "", /Platform Overview \[global\/global\]/);
  assert.match(memory.detail ?? "", /Provider Policy \[project\/global\]/);
});

test("formatActivityPayload preserves readable JSON formatting", () => {
  const text = formatActivityPayload({
    providerName: "OpenRouter",
    modelName: "demo-model",
    nested: { ok: true },
  });

  assert.match(text, /"providerName": "OpenRouter"/);
  assert.match(text, /"nested": \{\n\s+"ok": true\n\s+\}/);
});
