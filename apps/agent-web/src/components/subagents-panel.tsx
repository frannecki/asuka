"use client";

import { useEffect, useState } from "react";

import { createSubagent, listSubagents } from "@/lib/api";
import type { SubagentRecord } from "@/lib/types";
import { humanizeLabel } from "@/lib/view";

export function SubagentsPanel() {
  const [subagents, setSubagents] = useState<SubagentRecord[]>([]);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [scope, setScope] = useState("");
  const [maxSteps, setMaxSteps] = useState("6");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void listSubagents()
      .then((nextSubagents) => {
        if (!cancelled) {
          setSubagents(nextSubagents);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(
            loadError instanceof Error
              ? loadError.message
              : "Failed to load subagents.",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const created = await createSubagent({
        name,
        description,
        scope,
        maxSteps: Number(maxSteps),
      });
      setSubagents((current) => [created, ...current]);
      setName("");
      setDescription("");
      setScope("");
      setMaxSteps("6");
      setError(null);
    } catch (createError) {
      setError(
        createError instanceof Error
          ? createError.message
          : "Failed to create subagent.",
      );
    }
  }

  return (
    <div className="stack-gap">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Subagents</p>
            <h2>Register bounded specialists the runtime can delegate to.</h2>
          </div>
          <p>
            Subagents define scope and step budgets, then show up in execution
            views when the runtime delegates work.
          </p>
          {error ? <p className="error-copy">{error}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{subagents.length}</strong>
              <span>registered subagents</span>
            </article>
          </div>
        </div>
      </section>

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Register</p>
            <h2>Delegation registry</h2>
          </div>
        </div>
        <form className="form-grid" onSubmit={handleSubmit}>
          <label>
            Name
            <input
              className="text-input"
              onChange={(event) => setName(event.target.value)}
              value={name}
            />
          </label>
          <label>
            Description
            <textarea
              className="text-input"
              onChange={(event) => setDescription(event.target.value)}
              rows={3}
              value={description}
            />
          </label>
          <label>
            Scope
            <input
              className="text-input"
              onChange={(event) => setScope(event.target.value)}
              value={scope}
            />
          </label>
          <label>
            Max steps
            <input
              className="text-input"
              min="1"
              onChange={(event) => setMaxSteps(event.target.value)}
              type="number"
              value={maxSteps}
            />
          </label>
          <button className="primary-button" type="submit">
            Register subagent
          </button>
        </form>
      </section>

      <section className="catalog-grid">
        {subagents.map((subagent) => (
          <article className="catalog-card" key={subagent.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{humanizeLabel(subagent.status)}</p>
                <h3>{subagent.name}</h3>
              </div>
              <span className="status-pill">{subagent.maxSteps} steps</span>
            </div>
            <p>{subagent.description}</p>
            <p className="hint-copy">{subagent.scope}</p>
          </article>
        ))}
      </section>
    </div>
  );
}
