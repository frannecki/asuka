"use client";

import { useEffect, useState } from "react";

import { createSubagent, listSubagents } from "@/lib/api";
import type { SubagentRecord } from "@/lib/types";

export function SubagentsPanel() {
  const [subagents, setSubagents] = useState<SubagentRecord[]>([]);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [scope, setScope] = useState("");
  const [maxSteps, setMaxSteps] = useState("6");

  useEffect(() => {
    void listSubagents().then(setSubagents);
  }, []);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
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
  }

  return (
    <div className="stack-gap">
      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Subagents</p>
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

      <section className="grid-cards">
        {subagents.map((subagent) => (
          <article className="panel stack-gap" key={subagent.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{subagent.status}</p>
                <h2>{subagent.name}</h2>
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
