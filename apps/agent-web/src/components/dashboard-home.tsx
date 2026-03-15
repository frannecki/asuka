"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState, useTransition } from "react";

import {
  createSession,
  listMemoryDocuments,
  listProviders,
  listSessionArtifacts,
  listSessions,
  listSkillPresets,
  listTasks,
} from "@/lib/api";
import type {
  ArtifactRecord,
  MemoryDocumentRecord,
  ProviderAccountRecord,
  SessionRecord,
  SkillPreset,
  TaskRecord,
} from "@/lib/types";

type RecentArtifact = ArtifactRecord & {
  sessionTitle: string;
};

export function DashboardHome() {
  const router = useRouter();
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [providers, setProviders] = useState<ProviderAccountRecord[]>([]);
  const [memoryDocuments, setMemoryDocuments] = useState<MemoryDocumentRecord[]>([]);
  const [presets, setPresets] = useState<SkillPreset[]>([]);
  const [recentArtifacts, setRecentArtifacts] = useState<RecentArtifact[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const [nextSessions, nextTasks, nextProviders, nextMemory, nextPresets] =
          await Promise.all([
            listSessions(),
            listTasks(),
            listProviders(),
            listMemoryDocuments(),
            listSkillPresets(),
          ]);
        const recentSessionSet = nextSessions.slice(0, 4);
        const artifactGroups = await Promise.all(
          recentSessionSet.map(async (session) => ({
            session,
            artifacts: await listSessionArtifacts(session.id),
          })),
        );
        if (cancelled) {
          return;
        }

        const flattenedArtifacts = artifactGroups
          .flatMap(({ session, artifacts }) =>
            artifacts.map((artifact) => ({
              ...artifact,
              sessionTitle: session.title,
            })),
          )
          .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
          .slice(0, 6);

        startTransition(() => {
          setSessions(nextSessions);
          setTasks(nextTasks);
          setProviders(nextProviders);
          setMemoryDocuments(nextMemory);
          setPresets(nextPresets);
          setRecentArtifacts(flattenedArtifacts);
        });
      } catch (loadError) {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load dashboard data.",
        );
      }
    }

    void load();

    return () => {
      cancelled = true;
    };
  }, [startTransition]);

  async function handleCreateSession() {
    try {
      const session = await createSession("New workspace session");
      router.push(`/sessions/${session.id}/chat`);
    } catch (creationError) {
      setError(
        creationError instanceof Error
          ? creationError.message
          : "Failed to create a session.",
      );
    }
  }

  const activeTasks = tasks.filter((task) =>
    ["queued", "planning", "running", "waitingForApproval", "suspended"].includes(task.status),
  );
  const recentSessions = sessions.slice(0, 4);
  const recentTasks = tasks.slice(0, 5);
  const sessionMemoryCount = memoryDocuments.filter(
    (document) => document.memoryScope === "session",
  ).length;
  const projectMemoryCount = memoryDocuments.filter(
    (document) => document.memoryScope === "project",
  ).length;
  const globalMemoryCount = memoryDocuments.filter(
    (document) => document.memoryScope === "global",
  ).length;
  const pinnedMemoryCount = memoryDocuments.filter((document) => document.isPinned).length;

  return (
    <div className="dashboard-shell">
      <section className="dashboard-hero panel">
        <div className="dashboard-hero-copy">
          <p className="eyebrow">Dashboard</p>
          <h2>Modern local agent workspace for live runs, memory, and session-specific skills.</h2>
          <p>
            Resume recent sessions, inspect active harness work, browse generated
            artifacts, and jump into the next workspace without carrying the full
            execution console everywhere.
          </p>
        </div>
        <div className="dashboard-hero-actions">
          <button
            className="primary-button"
            disabled={isPending}
            onClick={handleCreateSession}
            type="button"
          >
            New session
          </button>
          <Link className="ghost-button" href="/sessions">
            Open sessions
          </Link>
        </div>
      </section>

      {error ? <p className="error-copy">{error}</p> : null}

      <section className="dashboard-grid">
        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Recent sessions</p>
              <h2>Resume workspaces</h2>
            </div>
            <span className="status-pill">{sessions.length} total</span>
          </div>
          <div className="dashboard-list">
            {recentSessions.map((session) => (
              <Link
                className="dashboard-row"
                href={`/sessions/${session.id}/chat`}
                key={session.id}
              >
                <div>
                  <strong>{session.title}</strong>
                  <p>{session.summary}</p>
                </div>
                <span>{session.lastRunAt ? "active" : "idle"}</span>
              </Link>
            ))}
            {recentSessions.length === 0 ? (
              <div className="empty-state small">
                No sessions yet. Create one to start the workspace flow.
              </div>
            ) : null}
          </div>
        </article>

        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Active work</p>
              <h2>In-flight tasks</h2>
            </div>
            <span className="status-pill">{activeTasks.length} active</span>
          </div>
          <div className="dashboard-list">
            {(activeTasks.length > 0 ? activeTasks : recentTasks).map((task) => (
              <Link
                className="dashboard-row"
                href={`/sessions/${task.sessionId}/execution`}
                key={task.id}
              >
                <div>
                  <strong>{task.title}</strong>
                  <p>{task.summary}</p>
                </div>
                <span>{task.status}</span>
              </Link>
            ))}
            {recentTasks.length === 0 ? (
              <div className="empty-state small">
                No harness tasks yet. Post a message in any session to create
                the first task bundle.
              </div>
            ) : null}
          </div>
        </article>
      </section>

      <section className="dashboard-grid">
        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Recent artifacts</p>
              <h2>Latest workspace outputs</h2>
            </div>
            <Link className="ghost-button" href="/sessions">
              Browse sessions
            </Link>
          </div>
          <div className="dashboard-list">
            {recentArtifacts.map((artifact) => (
              <Link
                className="dashboard-row"
                href={`/sessions/${artifact.sessionId}/artifacts`}
                key={artifact.id}
              >
                <div>
                  <strong>{artifact.displayName}</strong>
                  <p>{artifact.sessionTitle}</p>
                </div>
                <span>{artifact.kind}</span>
              </Link>
            ))}
            {recentArtifacts.length === 0 ? (
              <div className="empty-state small">
                Completed runs will surface durable reports, markdown, and data
                artifacts here.
              </div>
            ) : null}
          </div>
        </article>

        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Memory surface</p>
              <h2>Retrieval-ready corpus</h2>
            </div>
            <Link className="ghost-button" href="/memory">
              Open memory
            </Link>
          </div>
          <div className="dashboard-stats-grid">
            <article className="dashboard-stat-card">
              <strong>{sessionMemoryCount}</strong>
              <span>session-scoped</span>
            </article>
            <article className="dashboard-stat-card">
              <strong>{projectMemoryCount}</strong>
              <span>project-scoped</span>
            </article>
            <article className="dashboard-stat-card">
              <strong>{globalMemoryCount}</strong>
              <span>global-scoped</span>
            </article>
            <article className="dashboard-stat-card">
              <strong>{pinnedMemoryCount}</strong>
              <span>pinned notes</span>
            </article>
          </div>
          <p className="hint-copy">
            Session memory feeds short-term recall, while project/global
            documents back the cross-session retrieval path.
          </p>
        </article>
      </section>

      <section className="dashboard-grid">
        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Skill presets</p>
              <h2>Ready-made session profiles</h2>
            </div>
            <Link className="ghost-button" href="/skills">
              Global skills
            </Link>
          </div>
          <div className="preset-grid">
            {presets.map((preset) => (
              <article className="preset-card" key={preset.id}>
                <div>
                  <p className="eyebrow">{preset.id}</p>
                  <h3>{preset.title}</h3>
                </div>
                <p>{preset.description}</p>
                <div className="session-chip-list">
                  {preset.skillNames.map((skillName) => (
                    <span className="timeline-chip" key={skillName}>
                      {skillName}
                    </span>
                  ))}
                </div>
              </article>
            ))}
          </div>
        </article>

        <article className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Providers</p>
              <h2>Configured backends</h2>
            </div>
            <Link className="ghost-button" href="/settings/providers">
              Manage
            </Link>
          </div>
          <div className="dashboard-provider-list">
            {providers.map((provider) => (
              <div className="provider-chip-card" key={provider.id}>
                <strong>{provider.displayName}</strong>
                <p>{provider.models.length} model(s)</p>
              </div>
            ))}
          </div>

          <div className="panel-header">
            <div>
              <p className="eyebrow">Control center</p>
              <h2>Global surfaces</h2>
            </div>
          </div>
          <div className="grid-cards compact-grid">
            <Link className="panel feature-card compact" href="/memory">
              <p className="eyebrow">Memory</p>
              <h2>Corpus & recall</h2>
              <p>Inspect local RAG documents and scoped memory retrieval.</p>
            </Link>
            <Link className="panel feature-card compact" href="/skills">
              <p className="eyebrow">Skills</p>
              <h2>Global registry</h2>
              <p>Manage reusable skills that sessions can opt into.</p>
            </Link>
            <Link className="panel feature-card compact" href="/subagents">
              <p className="eyebrow">Subagents</p>
              <h2>Delegation</h2>
              <p>Inspect bounded specialists and their runtime scopes.</p>
            </Link>
            <Link className="panel feature-card compact" href="/settings/mcp">
              <p className="eyebrow">MCP</p>
              <h2>Servers</h2>
              <p>Track registered MCP surfaces and capabilities.</p>
            </Link>
          </div>
        </article>
      </section>
    </div>
  );
}
