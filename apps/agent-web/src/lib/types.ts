export type SessionStatus = "active" | "archived";
export type ResourceStatus = "active" | "disabled";
export type RunStatus = "running" | "completed" | "failed" | "cancelled";
export type RunStreamStatus = "idle" | "active" | "completed" | "failed" | "cancelled";
export type MessageRole = "user" | "assistant" | "system";
export type MemoryScope = "session" | "project" | "global";
export type SessionSkillPolicyMode = "inheritDefault" | "preset" | "custom";
export type SessionSkillAvailability = "enabled" | "disabled" | "pinned";
export type TaskStatus =
  | "queued"
  | "planning"
  | "running"
  | "waitingForApproval"
  | "suspended"
  | "completed"
  | "failed"
  | "cancelled";
export type PlanStatus = "draft" | "active" | "superseded";
export type PlanStepKind =
  | "contextBuild"
  | "tool"
  | "mcpTool"
  | "mcpResource"
  | "subagent"
  | "memoryWrite"
  | "compression"
  | "evaluate"
  | "respond";
export type PlanStepStatus = "pending" | "running" | "completed" | "failed" | "skipped";
export type RunStepStatus = "running" | "completed" | "failed" | "cancelled";
export type WorkspaceEntryKind = "file" | "directory";
export type ArtifactKind = "report" | "response" | "data";
export type ArtifactRenderMode = "html" | "markdown" | "json" | "text";
export type ArtifactProducerKind = "run" | "runStep" | "toolInvocation";
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
  skillSummary: SessionSkillSummary;
  activeRunSummary: RunRecord | null;
  latestRunSummary: RunRecord | null;
  activeTaskSummary: TaskRecord | null;
  latestStreamCheckpointSummary: StreamCheckpointSummary | null;
};

export type SessionSkillPolicy = {
  sessionId: string;
  mode: SessionSkillPolicyMode;
  presetId: string | null;
  updatedAt: string;
};

export type SessionSkillBinding = {
  sessionId: string;
  skillId: string;
  availability: SessionSkillAvailability;
  orderIndex: number;
  notes: string | null;
  updatedAt: string;
};

export type SkillPreset = {
  id: string;
  title: string;
  description: string;
  skillNames: string[];
};

export type EffectiveSessionSkill = {
  skill: SkillRecord;
  availability: SessionSkillAvailability;
  isExplicit: boolean;
  isPreset: boolean;
  isPinned: boolean;
  orderIndex: number;
};

export type SessionSkillSummary = {
  policy: SessionSkillPolicy;
  effectiveSkillCount: number;
  pinnedSkills: SkillRecord[];
};

export type SessionSkillsDetail = {
  sessionId: string;
  policy: SessionSkillPolicy;
  bindings: SessionSkillBinding[];
  effectiveSkills: EffectiveSessionSkill[];
  presets: SkillPreset[];
};

export type RunRecord = {
  id: string;
  sessionId: string;
  taskId: string;
  triggerType: string;
  status: RunStatus;
  selectedProvider?: string | null;
  selectedModel?: string | null;
  startedAt: string;
  finishedAt: string | null;
  error: string | null;
  effectiveSkillNames: string[];
  pinnedSkillNames: string[];
  lastEventSequence: number;
  streamStatus: RunStreamStatus;
  activeStreamMessageId: string | null;
};

export type StreamCheckpointSummary = {
  runId: string;
  lastSequence: number;
  draftReplyText: string;
  updatedAt: string;
  activeStreamMessageId: string | null;
};

export type RunAccepted = {
  run: RunRecord;
  userMessage: MessageRecord;
};

export type ActiveRunEnvelope = {
  run: RunRecord | null;
};

export type TaskRecord = {
  id: string;
  sessionId: string;
  title: string;
  goal: string;
  status: TaskStatus;
  kind: string;
  originMessageId: string;
  currentPlanId: string | null;
  latestRunId: string | null;
  summary: string;
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
};

export type PlanRecord = {
  id: string;
  taskId: string;
  version: number;
  status: PlanStatus;
  source: string;
  planningModel: string | null;
  createdAt: string;
  supersededAt: string | null;
};

export type PlanStepRecord = {
  id: string;
  planId: string;
  ordinal: number;
  title: string;
  description: string;
  kind: PlanStepKind;
  status: PlanStepStatus;
  dependsOn: string[];
  skillId: string | null;
  subagentId: string | null;
  expectedOutputs: string[];
  acceptanceCriteria: string[];
};

export type PlanDetail = {
  plan: PlanRecord;
  steps: PlanStepRecord[];
};

export type ExecutionTimelineGroup = {
  id: string;
  run: RunRecord;
  runSteps: RunStepRecord[];
  toolInvocations: ToolInvocationRecord[];
  artifacts: ArtifactRecord[];
};

export type ArtifactGroupRecord = {
  id: string;
  taskId: string;
  runId: string;
  title: string;
  summary: string;
  primaryArtifactId: string | null;
  artifactIds: string[];
  createdAt: string;
};

export type LineageNodeKind =
  | "task"
  | "run"
  | "runStep"
  | "toolInvocation"
  | "artifact";

export type LineageNodeRecord = {
  id: string;
  kind: LineageNodeKind;
  label: string;
  status: string | null;
  refId: string | null;
};

export type LineageEdgeRecord = {
  from: string;
  to: string;
  relation: string;
};

export type TaskExecutionDetail = {
  task: TaskRecord;
  planDetail: PlanDetail | null;
  runs: RunRecord[];
  timelineGroups: ExecutionTimelineGroup[];
  artifactGroups: ArtifactGroupRecord[];
  lineageNodes: LineageNodeRecord[];
  lineageEdges: LineageEdgeRecord[];
};

export type RunStepRecord = {
  id: string;
  runId: string;
  taskId: string;
  planStepId: string | null;
  sequence: number;
  kind: PlanStepKind;
  title: string;
  status: RunStepStatus;
  inputSummary: string;
  outputSummary: string | null;
  startedAt: string;
  finishedAt: string | null;
  error: string | null;
};

export type ToolInvocationRecord = {
  id: string;
  runStepId: string;
  runId: string;
  toolName: string;
  toolSource: string;
  argumentsJson: unknown;
  resultJson: unknown;
  ok: boolean;
  startedAt: string;
  finishedAt: string;
  error: string | null;
};

export type ArtifactRecord = {
  id: string;
  sessionId: string;
  taskId: string;
  runId: string;
  path: string;
  displayName: string;
  description: string;
  kind: ArtifactKind;
  mediaType: string;
  renderMode: ArtifactRenderMode;
  sizeBytes: number;
  producerKind: ArtifactProducerKind | null;
  producerRefId: string | null;
  createdAt: string;
  updatedAt: string;
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
  memoryScope: MemoryScope;
  ownerSessionId: string | null;
  ownerTaskId: string | null;
  isPinned: boolean;
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
  memoryScope: MemoryScope;
  ownerSessionId: string | null;
  content: string;
  score: number;
};

export type MemorySearchResult = {
  hits: MemorySearchHit[];
};

export type SessionMemoryRetrievalRecord = {
  runId: string;
  taskId: string;
  timestamp: string;
  hits: MemorySearchHit[];
};

export type SessionMemoryOverview = {
  sessionId: string;
  shortTermSummary: string;
  scopedDocuments: MemoryDocumentRecord[];
  pinnedDocuments: MemoryDocumentRecord[];
  recentRetrievals: SessionMemoryRetrievalRecord[];
};

export type ReindexResult = {
  documents: number;
  chunks: number;
};

export type WorkspaceNode = {
  name: string;
  path: string;
  kind: WorkspaceEntryKind;
  size: number | null;
  updatedAt: string | null;
  children: WorkspaceNode[];
};

export type RunEventEnvelope = {
  eventType: string;
  runId: string;
  sessionId: string;
  timestamp: string;
  sequence: number;
  payload: Record<string, unknown>;
};

export type RunEventHistory = {
  runId: string;
  afterSequence: number | null;
  events: RunEventEnvelope[];
  lastSequence: number;
};
