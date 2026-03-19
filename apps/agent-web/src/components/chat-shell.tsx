"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useTransition,
} from "react";
import { usePathname, useRouter } from "next/navigation";

import {
  STREAM_EVENTS,
  buildRunEventsUrl,
  createSession,
  getSession,
  getSessionActiveRun,
  getSessionWorkspaceTree,
  getTaskExecution,
  listSessionArtifacts,
  listSessions,
  listTasks,
  postMessage,
} from "@/lib/api";
import type {
  ArtifactRecord,
  MessageRecord,
  RunEventEnvelope,
  RunStepRecord,
  SessionDetail,
  SessionRecord,
  TaskRecord,
  ToolInvocationRecord,
  WorkspaceNode,
} from "@/lib/types";
import {
  applyRunStreamEvent,
  createChatStreamState,
  disconnectRunStream,
} from "@/components/chat-stream-state";
import { SessionInspectorDrawer } from "@/components/session-inspector-drawer";
import { pickDefaultWorkspacePath } from "@/components/workspace-panel";
import {
  compactId,
  excerpt,
  formatModelLabel,
  formatTime,
  humanizeLabel,
  isStructuredText,
} from "@/lib/view";

type ChatShellProps = {
  initialSessionId?: string;
  routeMode?: "root" | "session";
};

type HarnessBundle = {
  tasks: TaskRecord[];
  selectedTaskId: string | null;
  runSteps: RunStepRecord[];
  toolInvocations: ToolInvocationRecord[];
  artifacts: ArtifactRecord[];
  selectedWorkspacePath: string | null;
};

export function ChatShell({
  initialSessionId,
  routeMode = "root",
}: ChatShellProps) {
  const router = useRouter();
  const pathname = usePathname();
  const streamRef = useRef<EventSource | null>(null);
  const reconnectTimerRef = useRef<number | null>(null);
  const streamStateRef = useRef(createChatStreamState());
  const taskSelectionRef = useRef<string | null>(null);
  const workspaceSelectionRef = useRef<string | null>(null);

  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(
    initialSessionId ?? null,
  );
  const [sessionDetail, setSessionDetail] = useState<SessionDetail | null>(null);
  const [messages, setMessages] = useState<MessageRecord[]>([]);
  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [runSteps, setRunSteps] = useState<RunStepRecord[]>([]);
  const [toolInvocations, setToolInvocations] = useState<ToolInvocationRecord[]>(
    [],
  );
  const [artifacts, setArtifacts] = useState<ArtifactRecord[]>([]);
  const [selectedWorkspacePath, setSelectedWorkspacePath] = useState<string | null>(
    null,
  );
  const [composer, setComposer] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [expandedMessages, setExpandedMessages] = useState<Record<string, boolean>>({});
  const [streamState, setStreamState] = useState(() => createChatStreamState());
  const [isSessionLoading, setIsSessionLoading] = useState(false);
  const [isPending, startTransition] = useTransition();
  const selectedSessionId = initialSessionId ?? activeSessionId ?? sessions[0]?.id ?? null;
  const listedSession =
    sessions.find((session) => session.id === selectedSessionId) ?? null;
  const selectedSession =
    sessionDetail?.session.id === selectedSessionId
      ? sessionDetail.session
      : listedSession;
  const selectedTask =
    tasks.find((task) => task.id === activeTaskId) ?? tasks[0] ?? null;
  const visibleArtifacts = artifacts.filter((artifact) =>
    activeTaskId ? artifact.taskId === activeTaskId : true,
  );
  const { activity, draftReply, modelLabel, status } = streamState;
  const persistedModelLabel = formatModelLabel(
    sessionDetail?.activeRunSummary?.selectedProvider ??
      sessionDetail?.latestRunSummary?.selectedProvider,
    sessionDetail?.activeRunSummary?.selectedModel ??
      sessionDetail?.latestRunSummary?.selectedModel,
  );
  const displayModelLabel = modelLabel ?? persistedModelLabel;
  const displayStatus =
    status !== "idle"
      ? status
      : selectedTask?.status ??
        sessionDetail?.activeTaskSummary?.status ??
        sessionDetail?.latestRunSummary?.status ??
        "idle";

  const applyStreamState = useCallback((nextState: typeof streamStateRef.current) => {
    streamStateRef.current = nextState;
    setStreamState(nextState);
  }, []);

  const clearReconnectTimer = useCallback(() => {
    if (reconnectTimerRef.current !== null) {
      window.clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
  }, []);

  const refreshSessions = useCallback(async () => {
    try {
      const nextSessions = await listSessions();
      startTransition(() => {
        setSessions(nextSessions);
      });
    } catch (refreshError) {
      setError(
        refreshError instanceof Error
          ? refreshError.message
          : "Failed to load sessions.",
      );
    }
  }, [startTransition]);

  const loadSession = useCallback(
    async (sessionId: string) => {
      try {
        const detail = await getSession(sessionId);
        startTransition(() => {
          setSessionDetail(detail);
          setMessages(detail.messages);
        });
        emitSessionUpdated(sessionId);
      } catch (loadError) {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load the session.",
        );
      }
    },
    [startTransition],
  );

  const loadTaskSelection = useCallback(
    async (task: TaskRecord | null, runIdOverride?: string | null) => {
      if (!task) {
        startTransition(() => {
          setRunSteps([]);
          setToolInvocations([]);
          setActiveTaskId(null);
        });
        return;
      }

      try {
        const detail = await getTaskExecution(task.id);
        const selectedGroup =
          detail.timelineGroups.find((group) => group.run.id === runIdOverride) ??
          detail.timelineGroups.find((group) => group.run.id === task.latestRunId) ??
          detail.timelineGroups[0] ??
          null;

        startTransition(() => {
          setActiveTaskId(task.id);
          setRunSteps(selectedGroup?.runSteps ?? []);
          setToolInvocations(selectedGroup?.toolInvocations ?? []);
        });
      } catch (loadError) {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load harness state.",
        );
      }
    },
    [startTransition],
  );

  const loadHarness = useCallback(
    async (
      sessionId: string,
      preferredTaskId?: string | null,
      preferredRunId?: string | null,
    ) => {
      try {
        const bundle = await fetchHarnessBundle(
          sessionId,
          preferredTaskId,
          preferredRunId,
          taskSelectionRef.current,
          workspaceSelectionRef.current,
        );
        startTransition(() => {
          setTasks(bundle.tasks);
          setActiveTaskId(bundle.selectedTaskId);
          setRunSteps(bundle.runSteps);
          setToolInvocations(bundle.toolInvocations);
          setArtifacts(bundle.artifacts);
          setSelectedWorkspacePath(bundle.selectedWorkspacePath);
        });
      } catch (loadError) {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load harness state.",
        );
      }
    },
    [startTransition],
  );

  const connectToRunStream = useCallback(
    function connectToRunStream(
      runId: string,
      sessionId: string,
      afterSequence?: number | null,
    ) {
      clearReconnectTimer();
      streamRef.current?.close();

      const source = new EventSource(buildRunEventsUrl(runId, afterSequence));
      streamRef.current = source;

      for (const eventName of STREAM_EVENTS) {
        source.addEventListener(eventName, (event) => {
          const envelope = JSON.parse(
            (event as MessageEvent<string>).data,
          ) as RunEventEnvelope;
          const transition = applyRunStreamEvent(
            streamStateRef.current,
            eventName,
            envelope,
          );
          const nextState = {
            activity: transition.activity,
            activeRunId: transition.activeRunId,
            draftReply: transition.draftReply,
            lastSequence: transition.lastSequence,
            modelLabel: transition.modelLabel,
            status: transition.status,
          };
          applyStreamState(nextState);

          if (transition.shouldCloseStream) {
            source.close();
          }
          if (transition.shouldRefreshSessions) {
            void refreshSessions();
          }
          if (
            selectedSessionId === envelope.sessionId &&
            shouldRefreshHarnessFromEvent(eventName)
          ) {
            void loadHarness(envelope.sessionId, null, envelope.runId);
          }
          if (transition.sessionToReload) {
            void loadSession(transition.sessionToReload);
            void loadHarness(transition.sessionToReload);
          }
        });
      }

      source.onerror = () => {
        const transition = disconnectRunStream(streamStateRef.current);
        const nextState = {
          activity: transition.activity,
          activeRunId: transition.activeRunId,
          draftReply: transition.draftReply,
          lastSequence: transition.lastSequence,
          modelLabel: transition.modelLabel,
          status: transition.status,
        };
        applyStreamState(nextState);
        if (transition.shouldCloseStream) {
          source.close();
        }
        if (transition.shouldReconnect) {
          reconnectTimerRef.current = window.setTimeout(() => {
            connectToRunStream(runId, sessionId, transition.lastSequence);
          }, 1000);
        }
      };
    },
    [
      applyStreamState,
      clearReconnectTimer,
      loadHarness,
      loadSession,
      refreshSessions,
      selectedSessionId,
    ],
  );

  useEffect(() => {
    streamStateRef.current = streamState;
  }, [streamState]);

  useEffect(() => {
    taskSelectionRef.current = activeTaskId;
    workspaceSelectionRef.current = selectedWorkspacePath;
  }, [activeTaskId, selectedWorkspacePath]);

  useEffect(() => {
    let cancelled = false;

    void listSessions()
      .then((nextSessions) => {
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setSessions(nextSessions);
          if (!initialSessionId && !activeSessionId && nextSessions[0]) {
            setActiveSessionId(nextSessions[0].id);
          }
        });
      })
      .catch((refreshError: unknown) => {
        if (cancelled) {
          return;
        }

        setError(
          refreshError instanceof Error
            ? refreshError.message
            : "Failed to load sessions.",
        );
      });

    return () => {
      cancelled = true;
      clearReconnectTimer();
      streamRef.current?.close();
    };
  }, [activeSessionId, clearReconnectTimer, initialSessionId, startTransition]);

  useEffect(() => {
    clearReconnectTimer();
    streamRef.current?.close();
    applyStreamState(createChatStreamState());

    if (!selectedSessionId) {
      setIsSessionLoading(false);
      startTransition(() => {
        setSessionDetail(null);
        setMessages([]);
        setTasks([]);
        setActiveTaskId(null);
        setRunSteps([]);
        setToolInvocations([]);
        setArtifacts([]);
        setSelectedWorkspacePath(null);
        setExpandedMessages({});
      });
      return;
    }

    let cancelled = false;
    setIsSessionLoading(true);

    startTransition(() => {
      setMessages([]);
      setTasks([]);
      setActiveTaskId(null);
      setRunSteps([]);
      setToolInvocations([]);
      setArtifacts([]);
      setSelectedWorkspacePath(null);
      setExpandedMessages({});
    });

    void Promise.all([
      getSession(selectedSessionId),
      fetchHarnessBundle(
        selectedSessionId,
        null,
        null,
        taskSelectionRef.current,
        workspaceSelectionRef.current,
      ),
    ])
      .then(([detail, bundle]) => {
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setSessionDetail(detail);
          setMessages(detail.messages);
          setExpandedMessages({});
          setTasks(bundle.tasks);
          setActiveTaskId(bundle.selectedTaskId);
          setRunSteps(bundle.runSteps);
          setToolInvocations(bundle.toolInvocations);
          setArtifacts(bundle.artifacts);
          setSelectedWorkspacePath(bundle.selectedWorkspacePath);
        });
        setIsSessionLoading(false);
      })
      .catch((loadError: unknown) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load the session workspace.",
        );
        setIsSessionLoading(false);
      });

    void getSessionActiveRun(selectedSessionId)
      .then(({ run }) => {
        if (cancelled || !run) {
          return;
        }

        applyStreamState(
          createChatStreamState({
            activeRunId: run.id,
            status: "recovering",
          }),
        );
        connectToRunStream(run.id, run.sessionId, 0);
      })
      .catch((loadError: unknown) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to recover the active run stream.",
        );
      });

    return () => {
      cancelled = true;
      clearReconnectTimer();
      streamRef.current?.close();
    };
  }, [
    applyStreamState,
    clearReconnectTimer,
    connectToRunStream,
    selectedSessionId,
    startTransition,
  ]);

  function navigateToSession(sessionId: string) {
    setActiveSessionId(sessionId);
    const destination = `/sessions/${sessionId}/chat`;
    if (pathname !== destination) {
      router.push(destination);
    }
  }

  async function handleCreateSession() {
    try {
      const session = await createSession("New workspace session");
      setError(null);
      startTransition(() => {
        setSessions((current) => [session, ...current]);
        setSessionDetail(null);
        setMessages([]);
        setTasks([]);
        setActiveTaskId(null);
        setRunSteps([]);
        setToolInvocations([]);
        setArtifacts([]);
        setSelectedWorkspacePath(null);
        setExpandedMessages({});
      });
      navigateToSession(session.id);
    } catch (creationError) {
      setError(
        creationError instanceof Error
          ? creationError.message
          : "Failed to create a session.",
      );
    }
  }

  async function ensureSession() {
    if (selectedSessionId) {
      return selectedSessionId;
    }

    const session = await createSession("Fresh chat session");
    startTransition(() => {
      setSessions((current) => [session, ...current]);
      setSessionDetail(null);
      setMessages([]);
    });
    navigateToSession(session.id);
    return session.id;
  }

  async function handleSendMessage(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const content = composer.trim();
    if (!content) {
      return;
    }

    try {
      const sessionId = await ensureSession();
      const accepted = await postMessage(sessionId, content);
      const nextStreamState = createChatStreamState({
        activeRunId: accepted.run.id,
        status: "running",
      });

      setError(null);
      setComposer("");
      applyStreamState(nextStreamState);
      startTransition(() => {
        setMessages((current) => [...current, accepted.userMessage]);
      });
      void loadHarness(sessionId, accepted.run.taskId, accepted.run.id);

      connectToRunStream(accepted.run.id, sessionId, 0);
    } catch (sendError) {
      setError(
        sendError instanceof Error
          ? sendError.message
          : "Failed to send the message.",
      );
    }
  }

  function toggleMessage(messageId: string) {
    setExpandedMessages((current) => ({
      ...current,
      [messageId]: !current[messageId],
    }));
  }

  const activeSessionSummary = selectedSession
    ? excerpt(selectedSession.summary, 140)
    : "Pick a session from the list or create a fresh one to begin.";

  return (
    <div className="workspace-shell-outer stack-gap">
      <section className="command-shell">
        <aside className="command-sidebar">
          <div className="command-sidebar-head">
            <div>
              <p className="eyebrow">Session index</p>
              <h2>Open threads</h2>
            </div>
            <button className="ghost-button" onClick={handleCreateSession} type="button">
              New
            </button>
          </div>

          <div className="command-session-list">
            {sessions.map((session) => (
              <button
                className={`command-session-row${
                  session.id === selectedSessionId ? " is-active" : ""
                }`}
                key={session.id}
                onClick={() => navigateToSession(session.id)}
                type="button"
              >
                <div className="command-session-main">
                  <strong>{excerpt(session.title, 34)}</strong>
                  <span className="command-session-copy">
                    {excerpt(session.summary, 70)}
                  </span>
                </div>
                <div className="command-session-meta">
                  <span className={`command-status-dot is-${session.status}`} />
                  <span>{session.lastRunAt ? "Recent" : "New"}</span>
                </div>
              </button>
            ))}
          </div>
        </aside>

        <section className={`command-main${isSessionLoading ? " is-loading" : ""}`}>
          <div className="command-main-head">
            <div className="command-title-block">
              <p className="eyebrow">{routeMode === "root" ? "Control room" : "Session board"}</p>
              <h2>{selectedSession?.title ?? "No session selected"}</h2>
              <p>{activeSessionSummary}</p>
              {error ? <p className="error-copy command-error-inline">{error}</p> : null}
            </div>
            <div className="command-stats">
              <div className="command-stat">
                <span>Status</span>
                <strong>{humanizeLabel(displayStatus)}</strong>
              </div>
              <div className="command-stat">
                <span>Task</span>
                <strong>{selectedTask ? compactId(selectedTask.id) : "None"}</strong>
              </div>
              <div className="command-stat">
                <span>Messages</span>
                <strong>{messages.length + (draftReply ? 1 : 0)}</strong>
              </div>
              <div className="command-stat">
                <span>Outputs</span>
                <strong>{visibleArtifacts.length}</strong>
              </div>
            </div>
          </div>

          <div className="transcript command-transcript">
            {isSessionLoading ? (
              <div className="command-loading-stack" aria-hidden="true">
                <div className="message-skeleton is-right" />
                <div className="message-skeleton" />
                <div className="message-skeleton is-right" />
              </div>
            ) : (
              <>
                {messages.map((message) => (
                  <TranscriptMessage
                    expanded={Boolean(expandedMessages[message.id])}
                    key={message.id}
                    message={message}
                    onToggle={() => toggleMessage(message.id)}
                  />
                ))}

                {draftReply ? (
                  <article className="message-bubble role-assistant">
                    <div className="message-meta">
                      <span className="message-kicker">assistant</span>
                      <time>streaming</time>
                    </div>
                    <p className="message-content">{draftReply}</p>
                  </article>
                ) : null}

                {messages.length === 0 && !draftReply ? (
                  <div className="empty-state">
                    Select a session on the left or create a new one, then send a
                    prompt. The center panel stays focused on the transcript only.
                  </div>
                ) : null}
              </>
            )}
          </div>

          <form className="composer command-composer" onSubmit={handleSendMessage}>
            <textarea
              className="composer-input"
              id="chat-composer"
              name="chat-composer"
              onChange={(event) => setComposer(event.target.value)}
              placeholder="Ask the agent to reason, call tools, inspect files, or delegate."
              rows={4}
              value={composer}
            />
            <div className="composer-actions">
              <div className="command-composer-meta">
                {displayModelLabel ? <span>{displayModelLabel}</span> : <span>No model yet</span>}
                <span>Structured replies collapse automatically</span>
              </div>
              <button className="primary-button" disabled={isPending} type="submit">
                Send prompt
              </button>
            </div>
          </form>
        </section>

        <SessionInspectorDrawer
          activeTaskId={activeTaskId}
          activity={activity}
          artifacts={visibleArtifacts}
          isLoading={isSessionLoading}
          modelLabel={displayModelLabel}
          onSelectPath={setSelectedWorkspacePath}
          onSelectTaskId={(taskId) => {
            const task = tasks.find((candidate) => candidate.id === taskId) ?? null;
            if (selectedSessionId && task) {
              void loadHarness(selectedSessionId, task.id, task.latestRunId);
              return;
            }
            void loadTaskSelection(task);
          }}
          runSteps={runSteps}
          selectedPath={selectedWorkspacePath}
          sessionId={selectedSessionId}
          status={displayStatus}
          tasks={tasks}
          toolInvocations={toolInvocations}
        />
      </section>
    </div>
  );
}

function TranscriptMessage({
  message,
  expanded,
  onToggle,
}: {
  message: MessageRecord;
  expanded: boolean;
  onToggle: () => void;
}) {
  const structured = isStructuredText(message.content);
  const collapsible =
    message.content.length > 560 || message.content.split(/\r?\n/).length > 10;
  const contentClass = structured ? "message-pre" : "message-content";
  const collapsedClass = collapsible && !expanded ? " is-collapsed" : "";

  return (
    <article className={`message-bubble role-${message.role}`}>
      <div className="message-meta">
        <span className="message-kicker">{message.role}</span>
        <time>{formatTime(message.createdAt)}</time>
      </div>

      {structured ? (
        <pre className={`${contentClass}${collapsedClass}`}>{message.content}</pre>
      ) : (
        <p className={`${contentClass}${collapsedClass}`}>{message.content}</p>
      )}

      {collapsible ? (
        <button className="message-expand" onClick={onToggle} type="button">
          {expanded ? "Collapse" : structured ? "Expand payload" : "Read more"}
        </button>
      ) : null}
    </article>
  );
}

async function fetchHarnessBundle(
  sessionId: string,
  preferredTaskId: string | null | undefined,
  preferredRunId: string | null | undefined,
  currentTaskId: string | null,
  currentWorkspacePath: string | null,
): Promise<HarnessBundle> {
  const [nextTasks, nextWorkspaceTree, nextArtifacts] = await Promise.all([
    listTasks(sessionId),
    getSessionWorkspaceTree(sessionId),
    listSessionArtifacts(sessionId),
  ]);
  const selectedTask =
    nextTasks.find((task) => task.id === preferredTaskId) ??
    nextTasks.find((task) => task.id === currentTaskId) ??
    nextTasks[0] ??
    null;
  const filteredArtifacts = selectedTask
    ? nextArtifacts.filter((artifact) => artifact.taskId === selectedTask.id)
    : nextArtifacts;
  const executionDetail = selectedTask
    ? await getTaskExecution(selectedTask.id)
    : null;
  const selectedGroup =
    executionDetail?.timelineGroups.find((group) => group.run.id === preferredRunId) ??
    executionDetail?.timelineGroups.find(
      (group) => group.run.id === selectedTask?.latestRunId,
    ) ??
    executionDetail?.timelineGroups[0] ??
    null;
  const currentPathStillVisible =
    currentWorkspacePath &&
    containsWorkspacePath(nextWorkspaceTree, currentWorkspacePath) &&
    (filteredArtifacts.length === 0 ||
      filteredArtifacts.some((artifact) => artifact.path === currentWorkspacePath));
  const nextSelectedWorkspacePath =
    currentPathStillVisible
      ? currentWorkspacePath
      : filteredArtifacts[0]?.path ?? pickDefaultWorkspacePath(nextWorkspaceTree);

  return {
    tasks: nextTasks,
    selectedTaskId: selectedTask?.id ?? null,
    runSteps: selectedGroup?.runSteps ?? [],
    toolInvocations: selectedGroup?.toolInvocations ?? [],
    artifacts: nextArtifacts,
    selectedWorkspacePath: nextSelectedWorkspacePath,
  };
}

function containsWorkspacePath(tree: WorkspaceNode | null, path: string): boolean {
  if (!tree) {
    return false;
  }
  if (tree.path === path) {
    return true;
  }

  return tree.children.some((child) => containsWorkspacePath(child, path));
}

function shouldRefreshHarnessFromEvent(eventType: string): boolean {
  return [
    "run.step.started",
    "tool.call.completed",
    "subagent.completed",
    "memory.written",
    "run.completed",
    "run.failed",
  ].includes(eventType);
}

function emitSessionUpdated(sessionId: string) {
  window.dispatchEvent(
    new CustomEvent("asuka:session-updated", {
      detail: { sessionId },
    }),
  );
}
