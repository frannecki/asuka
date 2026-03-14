"use client";

import { useEffect, useState } from "react";

import { createSkill, listSkills } from "@/lib/api";
import type { SkillRecord } from "@/lib/types";

export function SkillsPanel() {
  const [skills, setSkills] = useState<SkillRecord[]>([]);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");

  useEffect(() => {
    void listSkills().then(setSkills);
  }, []);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const created = await createSkill({ name, description });
    setSkills((current) => [created, ...current]);
    setName("");
    setDescription("");
  }

  return (
    <div className="stack-gap">
      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Skills</p>
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

      <section className="grid-cards">
        {skills.map((skill) => (
          <article className="panel stack-gap" key={skill.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{skill.status}</p>
                <h2>{skill.name}</h2>
              </div>
            </div>
            <p>{skill.description}</p>
          </article>
        ))}
      </section>
    </div>
  );
}
