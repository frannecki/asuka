import type {
  CapabilityEnvelope,
  MemoryDocumentDetail,
  MemoryDocumentRecord,
  MemorySearchResult,
  McpServerRecord,
  ProviderAccountRecord,
  ReindexResult,
  RunAccepted,
  SessionDetail,
  SessionRecord,
  SkillRecord,
  SubagentRecord,
  TestResult,
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

export function buildRunEventsUrl(runId: string): string {
  return `${API_BASE}/api/v1/runs/${runId}/events`;
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

export function postMessage(sessionId: string, content: string) {
  return apiFetch<RunAccepted>(`/api/v1/sessions/${sessionId}/messages`, {
    method: "POST",
    body: { content },
  });
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

export function searchMemory(payload: {
  query: string;
  namespace?: string | null;
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
