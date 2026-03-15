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
  getSessionActiveRun,
  getSessionWorkspaceTree,
  getSession,
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
import {
  pickDefaultWorkspacePath,
} from "@/components/workspace-panel";

type ChatShellProps = {
  initialSessionId?: string;
  routeMode?: "legacy" | "session";
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
  routeMode = "legacy",
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
  const [streamState, setStreamState] = useState(() => createChatStreamState());
  const [isPending, startTransition] = useTransition();
  const showSessionRail = routeMode === "legacy";
  const selectedSessionId = initialSessionId ?? activeSessionId ?? sessions[0]?.id ?? null;
  const { activity, draftReply, modelLabel, status } = streamState;

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
          setMessages(detail.messages);
        });
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
  }, [clearReconnectTimer, startTransition]);

  useEffect(() => {
    clearReconnectTimer();
    streamRef.current?.close();
    applyStreamState(createChatStreamState());

    if (!selectedSessionId) {
      return;
    }

    let cancelled = false;

    void getSession(selectedSessionId)
      .then((detail) => {
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setMessages(detail.messages);
        });
      })
      .catch((loadError: unknown) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load the session.",
        );
      });
    void fetchHarnessBundle(
      selectedSessionId,
      null,
      null,
      taskSelectionRef.current,
      workspaceSelectionRef.current,
    )
      .then((bundle) => {
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setTasks(bundle.tasks);
          setActiveTaskId(bundle.selectedTaskId);
          setRunSteps(bundle.runSteps);
          setToolInvocations(bundle.toolInvocations);
          setArtifacts(bundle.artifacts);
          setSelectedWorkspacePath(bundle.selectedWorkspacePath);
        });
      })
      .catch((loadError: unknown) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load harness state.",
        );
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
    const destination =
      routeMode === "session" ? `/sessions/${sessionId}/chat` : `/chat/${sessionId}`;
    if (pathname !== destination) {
      router.push(destination);
    }
  }

  async function handleCreateSession() {
    try {
      const session = await createSession("New orchestration session");
      setError(null);
      startTransition(() => {
        setSessions((current) => [session, ...current]);
        setMessages([]);
        setTasks([]);
        setActiveTaskId(null);
        setRunSteps([]);
        setToolInvocations([]);
        setArtifacts([]);
        setSelectedWorkspacePath(null);
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

  return (
    <div className={`chat-layout${showSessionRail ? "" : " chat-layout-session"}`}>
      {showSessionRail ? (
        <aside className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Sessions</p>
              <h2>Conversation state</h2>
            </div>
            <button className="ghost-button" onClick={handleCreateSession}>
              New
            </button>
          </div>
          <div className="session-list">
            {sessions.map((session) => (
              <button
                key={session.id}
                className={`session-card${
                  session.id === selectedSessionId ? " is-active" : ""
                }`}
                onClick={() => navigateToSession(session.id)}
                type="button"
              >
                <strong>{session.title}</strong>
                <span>{session.summary}</span>
              </button>
            ))}
            {sessions.length === 0 ? (
              <div className="empty-state small">
                No sessions yet. Start one to exercise the agent loop.
              </div>
            ) : null}
          </div>
        </aside>
      ) : null}

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Chat</p>
            <h2>Session transcript</h2>
          </div>
          <div className="stack-inline">
            {modelLabel ? <div className="status-pill">{modelLabel}</div> : null}
            <div className="status-pill">{status}</div>
          </div>
        </div>

        {routeMode === "session" ? (
          <div className="chat-view-topline">
            <p className="hint-copy">
              Keep the conversation centered here. Use the inspector for live
              activity, latest run details, and recent artifacts, then switch to
              the dedicated execution or artifacts routes for the full harness view.
            </p>
          </div>
        ) : null}

        <div className="transcript">
          {messages.map((message) => (
            <article
              key={message.id}
              className={`message-bubble role-${message.role}`}
            >
              <header>
                <span>{message.role}</span>
                <time>{new Date(message.createdAt).toLocaleTimeString()}</time>
              </header>
              <p>{message.content}</p>
            </article>
          ))}

          {draftReply ? (
            <article className="message-bubble role-assistant">
              <header>
                <span>assistant</span>
                <time>streaming</time>
              </header>
              <p>{draftReply}</p>
            </article>
          ) : null}

          {messages.length === 0 && !draftReply ? (
            <div className="empty-state">
              The backend seeds one starter session, but you can create a fresh
              one and begin streaming runs immediately.
            </div>
          ) : null}
        </div>

        <form className="composer" onSubmit={handleSendMessage}>
          <textarea
            className="composer-input"
            id="chat-composer"
            name="chat-composer"
            onChange={(event) => setComposer(event.target.value)}
            placeholder="Ask the agent to reason, call tools, or delegate to a subagent."
            rows={4}
            value={composer}
          />
          <div className="composer-actions">
            {error ? <p className="error-copy">{error}</p> : <span />}
            <button className="primary-button" disabled={isPending} type="submit">
              Send
            </button>
          </div>
        </form>
      </section>

      <SessionInspectorDrawer
        activeTaskId={activeTaskId}
        activity={activity}
        artifacts={artifacts.filter((artifact) =>
          activeTaskId ? artifact.taskId === activeTaskId : true,
        )}
        modelLabel={modelLabel}
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
        status={status}
        tasks={tasks}
        toolInvocations={toolInvocations}
      />
    </div>
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
