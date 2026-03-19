"use client";

import { useDeferredValue, useEffect, useState } from "react";

import {
  applySessionSkillPreset,
  getSessionSkills,
  listSkills,
  replaceSessionSkills,
  updateSessionSkillBinding,
} from "@/lib/api";
import type {
  SessionSkillAvailability,
  SessionSkillBinding,
  SessionSkillPolicyMode,
  SessionSkillsDetail,
  SkillRecord,
} from "@/lib/types";
import { humanizeLabel } from "@/lib/view";

type SessionSkillsConfiguratorProps = {
  sessionId: string;
};

export function SessionSkillsConfigurator({
  sessionId,
}: SessionSkillsConfiguratorProps) {
  const [detail, setDetail] = useState<SessionSkillsDetail | null>(null);
  const [registry, setRegistry] = useState<SkillRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query);

  useEffect(() => {
    let cancelled = false;

    void Promise.all([getSessionSkills(sessionId), listSkills()])
      .then(([nextDetail, nextRegistry]) => {
        if (cancelled) {
          return;
        }

        setDetail(nextDetail);
        setRegistry(nextRegistry);
      })
      .catch((loadError: unknown) => {
        if (cancelled) {
          return;
        }

        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load session skills.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  async function setMode(mode: SessionSkillPolicyMode) {
    if (!detail) {
      return;
    }

    setBusyKey(`mode:${mode}`);
    try {
      const fallbackPresetId = detail.policy.presetId ?? detail.presets[0]?.id ?? null;
      const nextDetail = await replaceSessionSkills(sessionId, {
        mode,
        presetId: mode === "preset" ? fallbackPresetId : null,
        bindings: detail.bindings.map(bindingToPayload),
      });
      setDetail(nextDetail);
      emitSessionSkillsUpdated(sessionId);
      setError(null);
    } catch (updateError) {
      setError(
        updateError instanceof Error
          ? updateError.message
          : "Failed to update the session skill policy.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handleApplyPreset(presetId: string) {
    setBusyKey(`preset:${presetId}`);
    try {
      const nextDetail = await applySessionSkillPreset(sessionId, presetId);
      setDetail(nextDetail);
      emitSessionSkillsUpdated(sessionId);
      setError(null);
    } catch (updateError) {
      setError(
        updateError instanceof Error
          ? updateError.message
          : "Failed to apply the preset.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  async function handleSetAvailability(
    skillId: string,
    availability: SessionSkillAvailability,
  ) {
    setBusyKey(`skill:${skillId}:${availability}`);
    try {
      const nextDetail = await updateSessionSkillBinding(sessionId, skillId, {
        availability,
      });
      setDetail(nextDetail);
      emitSessionSkillsUpdated(sessionId);
      setError(null);
    } catch (updateError) {
      setError(
        updateError instanceof Error
          ? updateError.message
          : "Failed to update the session skill.",
      );
    } finally {
      setBusyKey(null);
    }
  }

  const pinnedSkills =
    detail?.effectiveSkills.filter((entry) => entry.isPinned) ?? [];
  const normalizedQuery = deferredQuery.trim().toLowerCase();
  const visibleRegistry = registry.filter((skill) =>
    normalizedQuery.length === 0
      ? true
      : `${skill.name} ${skill.description}`.toLowerCase().includes(normalizedQuery),
  );

  return (
    <div className="stack-gap">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Session skills</p>
            <h2>Decide what this workspace is allowed to be good at.</h2>
          </div>
          <p>
            The frontend edits a session-scoped policy, explicit bindings, and
            optional presets without touching the global skill registry.
          </p>
          <div className="policy-mode-row">
            {(["inheritDefault", "preset", "custom"] as const).map((mode) => (
              <button
                className={`policy-chip${detail?.policy.mode === mode ? " is-active" : ""}`}
                disabled={busyKey !== null}
                key={mode}
                onClick={() => void setMode(mode)}
                type="button"
              >
                {humanizeLabel(mode)}
              </button>
            ))}
          </div>
          {error ? <p className="error-copy">{error}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{detail?.effectiveSkills.length ?? 0}</strong>
              <span>effective skills</span>
            </article>
            <article className="hero-stat">
              <strong>{pinnedSkills.length}</strong>
              <span>pinned skills</span>
            </article>
          </div>
          <article className="hero-orb">
            <p className="eyebrow">Pinned first</p>
            <strong>Promote the few skills that should stay at the front of the queue.</strong>
            <div className="session-chip-list">
              {pinnedSkills.map((entry) => (
                <span className="timeline-chip artifact" key={entry.skill.id}>
                  {entry.skill.name}
                </span>
              ))}
              {pinnedSkills.length === 0 ? (
                <span className="story-kicker">No pinned skills yet.</span>
              ) : null}
            </div>
          </article>
        </div>
      </section>

      <div className="session-skills-layout">
        <section className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Effective set</p>
              <h2>What the session can currently use</h2>
            </div>
          </div>

          <div className="session-chip-list">
            {detail?.effectiveSkills.map((entry) => (
              <span className="timeline-chip artifact" key={entry.skill.id}>
                {entry.skill.name}
              </span>
            ))}
            {detail?.effectiveSkills.length === 0 ? (
              <span className="story-kicker">
                No effective skills. Switch policy mode or apply a preset.
              </span>
            ) : null}
          </div>

          <div className="preset-grid">
            {detail?.presets.map((preset) => (
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
                <button
                  className="ghost-button"
                  disabled={busyKey !== null}
                  onClick={() => void handleApplyPreset(preset.id)}
                  type="button"
                >
                  Apply preset
                </button>
              </article>
            ))}
          </div>
        </section>

        <section className="panel stack-gap">
          <div className="panel-header">
            <div>
              <p className="eyebrow">Registry overrides</p>
              <h2>Per-session skill controls</h2>
            </div>
          </div>

          <label className="session-skill-search">
            Search skills
            <input
              className="text-input"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Filter by name or description"
              value={query}
            />
          </label>

          <div className="session-skill-list">
            {visibleRegistry.map((skill) => {
              const effective =
                detail?.effectiveSkills.find((entry) => entry.skill.id === skill.id) ?? null;
              const explicit =
                detail?.bindings.find((binding) => binding.skillId === skill.id) ?? null;
              const currentAvailability =
                explicit?.availability ??
                effective?.availability ??
                (detail?.policy.mode === "custom" ? "disabled" : "enabled");

              return (
                <article className="session-skill-card" key={skill.id}>
                  <div className="activity-copy">
                    <strong>{skill.name}</strong>
                    <p>{skill.description}</p>
                  </div>
                  <div className="status-strip">
                    <span className="status-pill">{humanizeLabel(currentAvailability)}</span>
                    {effective?.isPinned ? <span className="status-pill tone-sun">Pinned</span> : null}
                    {effective?.isPreset ? <span className="status-pill tone-sky">Preset</span> : null}
                    {effective?.isExplicit ? <span className="status-pill tone-mint">Override</span> : null}
                  </div>
                  <div className="policy-mode-row">
                    {(["enabled", "pinned", "disabled"] as const).map((availability) => (
                      <button
                        className={`policy-chip${
                          currentAvailability === availability ? " is-active" : ""
                        }`}
                        disabled={busyKey !== null}
                        key={availability}
                        onClick={() => void handleSetAvailability(skill.id, availability)}
                        type="button"
                      >
                        {humanizeLabel(availability)}
                      </button>
                    ))}
                  </div>
                </article>
              );
            })}
            {visibleRegistry.length === 0 ? (
              <div className="empty-state small">
                No skills matched your search. Try a different term or clear the filter.
              </div>
            ) : null}
          </div>
        </section>
      </div>
    </div>
  );
}

function bindingToPayload(binding: SessionSkillBinding) {
  return {
    skillId: binding.skillId,
    availability: binding.availability,
    orderIndex: binding.orderIndex,
    notes: binding.notes,
  };
}

function emitSessionSkillsUpdated(sessionId: string) {
  window.dispatchEvent(
    new CustomEvent("asuka:session-skills-updated", {
      detail: { sessionId },
    }),
  );
}
