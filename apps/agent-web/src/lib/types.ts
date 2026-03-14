export type SessionStatus = "active" | "archived";
export type ResourceStatus = "active" | "disabled";
export type RunStatus = "running" | "completed" | "failed" | "cancelled";
export type MessageRole = "user" | "assistant" | "system";
export type ProviderType =
  | "moonshot"
  | "openAi"
  | "anthropic"
  | "googleGemini"
  | "azureOpenAi"
  | "openRouter"
  | "xAi"
  | "custom";

export type SessionRecord = {
  id: string;
  title: string;
  status: SessionStatus;
  rootAgentId: string;
  createdAt: string;
  updatedAt: string;
  lastRunAt: string | null;
  summary: string;
};

export type MessageRecord = {
  id: string;
  sessionId: string;
  role: MessageRole;
  content: string;
  createdAt: string;
  runId: string | null;
};

export type SessionDetail = {
  session: SessionRecord;
  messages: MessageRecord[];
};

export type RunRecord = {
  id: string;
  sessionId: string;
  triggerType: string;
  status: RunStatus;
  selectedProvider?: string | null;
  selectedModel?: string | null;
  startedAt: string;
  finishedAt: string | null;
  error: string | null;
};

export type RunAccepted = {
  run: RunRecord;
  userMessage: MessageRecord;
};

export type ProviderModelRecord = {
  id: string;
  modelName: string;
  contextWindow: number;
  supportsTools: boolean;
  supportsEmbeddings: boolean;
  capabilities: string[];
  isDefault: boolean;
};

export type ProviderAccountRecord = {
  id: string;
  providerType: ProviderType;
  displayName: string;
  baseUrl: string | null;
  status: ResourceStatus;
  createdAt: string;
  updatedAt: string;
  models: ProviderModelRecord[];
};

export type SkillRecord = {
  id: string;
  name: string;
  description: string;
  status: ResourceStatus;
  createdAt: string;
  updatedAt: string;
};

export type SubagentRecord = {
  id: string;
  name: string;
  description: string;
  scope: string;
  maxSteps: number;
  status: ResourceStatus;
  createdAt: string;
  updatedAt: string;
};

export type McpServerRecord = {
  id: string;
  name: string;
  transport: string;
  command: string;
  status: ResourceStatus;
  capabilities: string[];
  createdAt: string;
  updatedAt: string;
};

export type TestResult = {
  ok: boolean;
  message: string;
};

export type CapabilityEnvelope = {
  capabilities: string[];
};

export type MemoryDocumentRecord = {
  id: string;
  title: string;
  namespace: string;
  source: string;
  content: string;
  summary: string;
  chunkCount: number;
  createdAt: string;
  updatedAt: string;
};

export type MemoryChunkRecord = {
  id: string;
  documentId: string;
  namespace: string;
  ordinal: number;
  content: string;
  keywords: string[];
};

export type MemoryDocumentDetail = {
  document: MemoryDocumentRecord;
  chunks: MemoryChunkRecord[];
};

export type MemorySearchHit = {
  documentId: string;
  chunkId: string;
  documentTitle: string;
  namespace: string;
  content: string;
  score: number;
};

export type MemorySearchResult = {
  hits: MemorySearchHit[];
};

export type ReindexResult = {
  documents: number;
  chunks: number;
};

export type RunEventEnvelope = {
  eventType: string;
  runId: string;
  sessionId: string;
  timestamp: string;
  sequence: number;
  payload: Record<string, unknown>;
};
