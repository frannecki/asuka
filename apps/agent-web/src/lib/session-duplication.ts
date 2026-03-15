"use client";

import {
  createMemoryDocument,
  createSession,
  getSessionMemoryOverview,
  getSessionSkills,
  replaceSessionSkills,
} from "@/lib/api";
import type { SessionRecord } from "@/lib/types";

export async function duplicateSessionWorkspace(
  sessionId: string,
  nextTitle: string,
): Promise<SessionRecord> {
  const [createdSession, skillsDetail, memoryOverview] = await Promise.all([
    createSession(nextTitle),
    getSessionSkills(sessionId),
    getSessionMemoryOverview(sessionId),
  ]);

  await replaceSessionSkills(createdSession.id, {
    mode: skillsDetail.policy.mode,
    presetId: skillsDetail.policy.presetId,
    bindings: skillsDetail.bindings.map((binding) => ({
      skillId: binding.skillId,
      availability: binding.availability,
      orderIndex: binding.orderIndex,
      notes: binding.notes,
    })),
  });

  await Promise.all(
    memoryOverview.scopedDocuments.map((document) =>
      createMemoryDocument({
        title: document.title,
        namespace: document.namespace,
        source: `session-duplicate:${sessionId}`,
        memoryScope: "session",
        ownerSessionId: createdSession.id,
        isPinned: document.isPinned,
        content: document.content,
      }),
    ),
  );

  return createdSession;
}
