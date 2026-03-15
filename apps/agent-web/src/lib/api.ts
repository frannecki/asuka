import type {
  ArtifactRecord,
  ActiveRunEnvelope,
  CapabilityEnvelope,
  MemoryDocumentDetail,
  MemoryDocumentRecord,
  MemorySearchResult,
  McpServerRecord,
  ProviderAccountRecord,
  PlanDetail,
  ReindexResult,
  RunAccepted,
  RunEventHistory,
  RunStepRecord,
  SessionDetail,
  SessionMemoryOverview,
  SessionSkillAvailability,
  SessionSkillPolicyMode,
  SessionSkillsDetail,
  SessionRecord,
  SkillPreset,
  SkillRecord,
  SubagentRecord,
  TaskRecord,
  TaskExecutionDetail,
  TestResult,
  ToolInvocationRecord,
  WorkspaceNode,
} from "@/lib/types";

const API_BASE =
  process.env.NEXT_PUBLIC_AGENT_API_BASE ?? "http://127.0.0.1:4000";

export const STREAM_EVENTS = [
  "run.stream.ready",
  "run.started",
  "model.selected",
  "run.step.started",
  "message.delta",
  "tool.call.started",
  "tool.call.completed",
  "subagent.started",
  "subagent.completed",
  "memory.retrieved",
  "memory.written",
  "run.completed",
  "run.failed",
] as const;

type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

type ApiInit = Omit<RequestInit, "body"> & {
  body?: JsonValue;
};

async function apiFetch<T>(
  path: string,
  init?: ApiInit,
): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
    body: init?.body ? JSON.stringify(init.body) : undefined,
    cache: "no-store",
  });

  if (!response.ok) {
    let message = `Request failed with ${response.status}`;

    try {
      const payload = (await response.json()) as { error?: string };
      if (payload.error) {
        message = payload.error;
      }
    } catch {
      // Ignore JSON parse issues here and surface the default status text.
    }

    throw new Error(message);
  }

  return (await response.json()) as T;
}

export function buildRunEventsUrl(
  runId: string,
  afterSequence?: number | null,
): string {
  const suffix =
    typeof afterSequence === "number"
      ? `?afterSequence=${encodeURIComponent(String(afterSequence))}`
      : "";
  return `${API_BASE}/api/v1/runs/${runId}/events${suffix}`;
}

function encodeWorkspacePath(path: string): string {
  return path
    .split("/")
    .filter((segment) => segment.length > 0)
    .map(encodeURIComponent)
    .join("/");
}

export function buildSessionWorkspaceRawUrl(
  sessionId: string,
  relativePath: string,
): string {
  return `${API_BASE}/api/v1/sessions/${sessionId}/workspace/raw/${encodeWorkspacePath(relativePath)}`;
}

export function buildSessionWorkspaceRenderUrl(
  sessionId: string,
  relativePath: string,
): string {
  return `${API_BASE}/api/v1/sessions/${sessionId}/workspace/render/${encodeWorkspacePath(relativePath)}`;
}

export function listSessions() {
  return apiFetch<SessionRecord[]>("/api/v1/sessions");
}

export function createSession(title?: string) {
  return apiFetch<SessionRecord>("/api/v1/sessions", {
    method: "POST",
    body: { title: title ?? null },
  });
}

export function getSession(sessionId: string) {
  return apiFetch<SessionDetail>(`/api/v1/sessions/${sessionId}`);
}

export function updateSession(
  sessionId: string,
  payload: {
    title?: string | null;
    status?: SessionRecord["status"] | null;
  },
) {
  return apiFetch<SessionRecord>(`/api/v1/sessions/${sessionId}`, {
    method: "PATCH",
    body: payload,
  });
}

export function getSessionActiveRun(sessionId: string) {
  return apiFetch<ActiveRunEnvelope>(`/api/v1/sessions/${sessionId}/active-run`);
}

export function postMessage(sessionId: string, content: string) {
  return apiFetch<RunAccepted>(`/api/v1/sessions/${sessionId}/messages`, {
    method: "POST",
    body: { content },
  });
}

export function getRunEventHistory(runId: string, afterSequence?: number | null) {
  const suffix =
    typeof afterSequence === "number"
      ? `?afterSequence=${encodeURIComponent(String(afterSequence))}`
      : "";
  return apiFetch<RunEventHistory>(
    `/api/v1/runs/${runId}/events/history${suffix}`,
  );
}

export function listTasks(sessionId?: string | null) {
  const query = sessionId ? `?sessionId=${encodeURIComponent(sessionId)}` : "";
  return apiFetch<TaskRecord[]>(`/api/v1/tasks${query}`);
}

export function getTaskPlan(taskId: string) {
  return apiFetch<PlanDetail>(`/api/v1/tasks/${taskId}/plan`);
}

export function getTaskExecution(taskId: string) {
  return apiFetch<TaskExecutionDetail>(`/api/v1/tasks/${taskId}/execution`);
}

export function listRunSteps(runId: string) {
  return apiFetch<RunStepRecord[]>(`/api/v1/runs/${runId}/steps`);
}

export function listToolInvocations(runId: string) {
  return apiFetch<ToolInvocationRecord[]>(
    `/api/v1/runs/${runId}/tool-invocations`,
  );
}

export function getSessionWorkspaceTree(sessionId: string) {
  return apiFetch<WorkspaceNode>(`/api/v1/sessions/${sessionId}/workspace/tree`);
}

export function listSessionArtifacts(sessionId: string) {
  return apiFetch<ArtifactRecord[]>(`/api/v1/sessions/${sessionId}/artifacts`);
}

export function listTaskArtifacts(taskId: string) {
  return apiFetch<ArtifactRecord[]>(`/api/v1/tasks/${taskId}/artifacts`);
}

export function listRunArtifacts(runId: string) {
  return apiFetch<ArtifactRecord[]>(`/api/v1/runs/${runId}/artifacts`);
}

export function listProviders() {
  return apiFetch<ProviderAccountRecord[]>("/api/v1/providers");
}

export function createProvider(payload: {
  providerType: ProviderAccountRecord["providerType"];
  displayName: string;
  baseUrl?: string | null;
}) {
  return apiFetch<ProviderAccountRecord>("/api/v1/providers", {
    method: "POST",
    body: payload,
  });
}

export function testProvider(providerId: string) {
  return apiFetch<TestResult>(`/api/v1/providers/${providerId}/test`, {
    method: "POST",
  });
}

export function syncProviderModels(providerId: string) {
  return apiFetch<ProviderAccountRecord>(
    `/api/v1/providers/${providerId}/models/sync`,
    {
      method: "POST",
    },
  );
}

export function listSkills() {
  return apiFetch<SkillRecord[]>("/api/v1/skills");
}

export function listSkillPresets() {
  return apiFetch<SkillPreset[]>("/api/v1/skill-presets");
}

export function getSessionSkills(sessionId: string) {
  return apiFetch<SessionSkillsDetail>(`/api/v1/sessions/${sessionId}/skills`);
}

export function replaceSessionSkills(
  sessionId: string,
  payload: {
    mode: SessionSkillPolicyMode;
    presetId?: string | null;
    bindings: {
      skillId: string;
      availability: SessionSkillAvailability;
      orderIndex?: number | null;
      notes?: string | null;
    }[];
  },
) {
  return apiFetch<SessionSkillsDetail>(`/api/v1/sessions/${sessionId}/skills`, {
    method: "PUT",
    body: payload,
  });
}

export function updateSessionSkillBinding(
  sessionId: string,
  skillId: string,
  payload: {
    availability: SessionSkillAvailability;
    orderIndex?: number | null;
    notes?: string | null;
  },
) {
  return apiFetch<SessionSkillsDetail>(
    `/api/v1/sessions/${sessionId}/skills/${skillId}`,
    {
      method: "PATCH",
      body: payload,
    },
  );
}

export function applySessionSkillPreset(sessionId: string, presetId: string) {
  return apiFetch<SessionSkillsDetail>(
    `/api/v1/sessions/${sessionId}/skills/apply-preset`,
    {
      method: "POST",
      body: { presetId },
    },
  );
}

export function createSkill(payload: {
  name: string;
  description: string;
}) {
  return apiFetch<SkillRecord>("/api/v1/skills", {
    method: "POST",
    body: payload,
  });
}

export function listSubagents() {
  return apiFetch<SubagentRecord[]>("/api/v1/subagents");
}

export function createSubagent(payload: {
  name: string;
  description: string;
  scope: string;
  maxSteps: number;
}) {
  return apiFetch<SubagentRecord>("/api/v1/subagents", {
    method: "POST",
    body: payload,
  });
}

export function listMcpServers() {
  return apiFetch<McpServerRecord[]>("/api/v1/mcp/servers");
}

export function createMcpServer(payload: {
  name: string;
  transport: string;
  command: string;
}) {
  return apiFetch<McpServerRecord>("/api/v1/mcp/servers", {
    method: "POST",
    body: payload,
  });
}

export function testMcpServer(serverId: string) {
  return apiFetch<TestResult>(`/api/v1/mcp/servers/${serverId}/test`, {
    method: "POST",
  });
}

export function getMcpCapabilities(serverId: string) {
  return apiFetch<CapabilityEnvelope>(
    `/api/v1/mcp/servers/${serverId}/capabilities`,
  );
}

export function listMemoryDocuments() {
  return apiFetch<MemoryDocumentRecord[]>("/api/v1/memory/documents");
}

export function createMemoryDocument(payload: {
  title: string;
  namespace?: string | null;
  source?: string | null;
  memoryScope?: "session" | "project" | "global" | null;
  ownerSessionId?: string | null;
  ownerTaskId?: string | null;
  isPinned?: boolean | null;
  content: string;
}) {
  return apiFetch<MemoryDocumentRecord>("/api/v1/memory/documents", {
    method: "POST",
    body: payload,
  });
}

export function getMemoryDocument(documentId: string) {
  return apiFetch<MemoryDocumentDetail>(`/api/v1/memory/documents/${documentId}`);
}

export function updateMemoryDocument(
  documentId: string,
  payload: {
    title?: string | null;
    namespace?: string | null;
    memoryScope?: "session" | "project" | "global" | null;
    ownerSessionId?: string | null;
    isPinned?: boolean | null;
  },
) {
  return apiFetch<MemoryDocumentRecord>(`/api/v1/memory/documents/${documentId}`, {
    method: "PATCH",
    body: payload,
  });
}

export async function deleteMemoryDocument(documentId: string) {
  const response = await fetch(`${API_BASE}/api/v1/memory/documents/${documentId}`, {
    method: "DELETE",
    cache: "no-store",
  });

  if (!response.ok) {
    let message = `Request failed with ${response.status}`;

    try {
      const payload = (await response.json()) as { error?: string };
      if (payload.error) {
        message = payload.error;
      }
    } catch {
      // Ignore JSON parse issues here and surface the default status text.
    }

    throw new Error(message);
  }
}

export function searchMemory(payload: {
  query: string;
  namespace?: string | null;
  memoryScopes?: Array<"session" | "project" | "global">;
  ownerSessionId?: string | null;
  limit?: number;
}) {
  return apiFetch<MemorySearchResult>("/api/v1/memory/search", {
    method: "POST",
    body: payload,
  });
}

export function reindexMemory() {
  return apiFetch<ReindexResult>("/api/v1/memory/reindex", {
    method: "POST",
  });
}

export function getSessionMemoryOverview(sessionId: string) {
  return apiFetch<SessionMemoryOverview>(`/api/v1/sessions/${sessionId}/memory`);
}

export function summarizeSessionMemory(sessionId: string) {
  return apiFetch<MemoryDocumentRecord>(
    `/api/v1/sessions/${sessionId}/memory/summarize`,
    {
      method: "POST",
    },
  );
}
