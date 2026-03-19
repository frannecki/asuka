"use client";

import { useEffect, useState } from "react";

import { createSkill, listSkills } from "@/lib/api";
import type { SkillRecord } from "@/lib/types";
import { humanizeLabel } from "@/lib/view";

export function SkillsPanel() {
  const [skills, setSkills] = useState<SkillRecord[]>([]);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void listSkills()
      .then((nextSkills) => {
        if (!cancelled) {
          setSkills(nextSkills);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(loadError instanceof Error ? loadError.message : "Failed to load skills.");
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const created = await createSkill({ name, description });
      setSkills((current) => [created, ...current]);
      setName("");
      setDescription("");
      setError(null);
    } catch (createError) {
      setError(
        createError instanceof Error ? createError.message : "Failed to create skill.",
      );
    }
  }

  return (
    <div className="stack-gap">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Skills</p>
            <h2>Build the reusable capability deck sessions can pull from.</h2>
          </div>
          <p>
            Skills stay global here, then individual sessions can inherit,
            override, pin, or disable them in their own configuration view.
          </p>
          {error ? <p className="error-copy">{error}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{skills.length}</strong>
              <span>registered skills</span>
            </article>
          </div>
        </div>
      </section>

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Register</p>
            <h2>Reusable capability bundles</h2>
          </div>
        </div>
        <form className="form-grid" onSubmit={handleSubmit}>
          <label>
            Skill name
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
          <button className="primary-button" type="submit">
            Register skill
          </button>
        </form>
      </section>

      <section className="catalog-grid">
        {skills.map((skill) => (
          <article className="catalog-card" key={skill.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{humanizeLabel(skill.status)}</p>
                <h3>{skill.name}</h3>
              </div>
            </div>
            <p>{skill.description}</p>
          </article>
        ))}
      </section>
    </div>
  );
}
