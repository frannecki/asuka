"use client";

import { useEffect, useRef, useState, useTransition } from "react";
import { usePathname, useRouter } from "next/navigation";

import {
  STREAM_EVENTS,
  buildRunEventsUrl,
  createSession,
  getSession,
  listSessions,
  postMessage,
} from "@/lib/api";
import type {
  MessageRecord,
  RunEventEnvelope,
  SessionRecord,
} from "@/lib/types";
import {
  applyRunStreamEvent,
  createChatStreamState,
  disconnectRunStream,
} from "@/components/chat-stream-state";
import { describeRunEvent } from "@/components/chat-activity";

type ChatShellProps = {
  initialSessionId?: string;
};

export function ChatShell({ initialSessionId }: ChatShellProps) {
  const router = useRouter();
  const pathname = usePathname();
  const streamRef = useRef<EventSource | null>(null);
  const streamStateRef = useRef(createChatStreamState());

  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(
    initialSessionId ?? null,
  );
  const [messages, setMessages] = useState<MessageRecord[]>([]);
  const [composer, setComposer] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [streamState, setStreamState] = useState(() => createChatStreamState());
  const [isPending, startTransition] = useTransition();
  const selectedSessionId = initialSessionId ?? activeSessionId ?? sessions[0]?.id ?? null;
  const { activity, draftReply, modelLabel, status } = streamState;

  const refreshSessions = async () => {
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
  };

  const loadSession = async (sessionId: string) => {
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
  };

  useEffect(() => {
    streamStateRef.current = streamState;
  }, [streamState]);

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
      streamRef.current?.close();
    };
  }, []);

  useEffect(() => {
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

    return () => {
      cancelled = true;
    };
  }, [selectedSessionId]);

  function navigateToSession(sessionId: string) {
    setActiveSessionId(sessionId);
    if (pathname !== `/chat/${sessionId}`) {
      router.push(`/chat/${sessionId}`);
    }
  }

  async function handleCreateSession() {
    try {
      const session = await createSession("New orchestration session");
      setError(null);
      startTransition(() => {
        setSessions((current) => [session, ...current]);
        setMessages([]);
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
      const nextStreamState = createChatStreamState({ status: "running" });

      setError(null);
      setComposer("");
      streamStateRef.current = nextStreamState;
      setStreamState(nextStreamState);
      startTransition(() => {
        setMessages((current) => [...current, accepted.userMessage]);
      });

      connectToRunStream(accepted.run.id);
    } catch (sendError) {
      setError(
        sendError instanceof Error
          ? sendError.message
          : "Failed to send the message.",
      );
    }
  }

  function connectToRunStream(runId: string) {
    streamRef.current?.close();

    const source = new EventSource(buildRunEventsUrl(runId));
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
          draftReply: transition.draftReply,
          modelLabel: transition.modelLabel,
          status: transition.status,
        };
        streamStateRef.current = nextState;
        setStreamState(nextState);

        if (transition.shouldCloseStream) {
          source.close();
        }
        if (transition.shouldRefreshSessions) {
          void refreshSessions();
        }
        if (transition.sessionToReload) {
          void loadSession(transition.sessionToReload);
        }
      });
    }

    source.onerror = () => {
      const transition = disconnectRunStream(streamStateRef.current);
      const nextState = {
        activity: transition.activity,
        draftReply: transition.draftReply,
        modelLabel: transition.modelLabel,
        status: transition.status,
      };
      streamStateRef.current = nextState;
      setStreamState(nextState);
      if (transition.shouldCloseStream) {
        source.close();
      }
    };
  }

  return (
    <div className="chat-layout">
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

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Chatbot</p>
            <h2>Runtime transcript</h2>
          </div>
          <div className="stack-inline">
            {modelLabel ? <div className="status-pill">{modelLabel}</div> : null}
            <div className="status-pill">{status}</div>
          </div>
        </div>

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

      <aside className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Inspector</p>
            <h2>Run activity</h2>
          </div>
        </div>

        <div className="activity-list">
          {activity.map((event) => {
            const descriptor = describeRunEvent(event);

            return (
              <article
                className={`activity-card tone-${descriptor.tone}`}
                key={event.sequence}
              >
                <div className="activity-topline">
                  <span className="activity-badge">{descriptor.badge}</span>
                  <span>{new Date(event.timestamp).toLocaleTimeString()}</span>
                </div>
                <div className="activity-copy">
                  <strong>{descriptor.title}</strong>
                  <p>{descriptor.summary}</p>
                </div>
                {descriptor.detail ? <pre>{descriptor.detail}</pre> : null}
              </article>
            );
          })}
          {activity.length === 0 ? (
            <div className="empty-state small">
              Tool calls, memory retrieval, and subagent events appear here.
            </div>
          ) : null}
        </div>
      </aside>
    </div>
  );
}
