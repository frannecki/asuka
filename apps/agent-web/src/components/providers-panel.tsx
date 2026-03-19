"use client";

import { useEffect, useState } from "react";

import {
  createProvider,
  listProviders,
  syncProviderModels,
  testProvider,
} from "@/lib/api";
import type { ProviderAccountRecord, ProviderType } from "@/lib/types";
import { humanizeLabel } from "@/lib/view";

const PROVIDER_OPTIONS: ProviderType[] = [
  "moonshot",
  "openAi",
  "anthropic",
  "googleGemini",
  "azureOpenAi",
  "openRouter",
  "xAi",
  "custom",
];

export function ProvidersPanel() {
  const [providers, setProviders] = useState<ProviderAccountRecord[]>([]);
  const [providerType, setProviderType] = useState<ProviderType>("openAi");
  const [displayName, setDisplayName] = useState("OpenAI");
  const [baseUrl, setBaseUrl] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void listProviders()
      .then((nextProviders) => {
        if (!cancelled) {
          setProviders(nextProviders);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(
            loadError instanceof Error
              ? loadError.message
              : "Failed to load providers.",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const created = await createProvider({
        providerType,
        displayName,
        baseUrl: baseUrl || null,
      });

      setProviders((current) => [created, ...current]);
      setFeedback(`Created provider account for ${created.displayName}.`);
      setError(null);
    } catch (createError) {
      setError(
        createError instanceof Error
          ? createError.message
          : "Failed to create provider.",
      );
    }
  }

  async function handleTest(providerId: string) {
    try {
      const result = await testProvider(providerId);
      setFeedback(result.message);
      setError(null);
    } catch (testError) {
      setError(
        testError instanceof Error
          ? testError.message
          : "Failed to test provider.",
      );
    }
  }

  async function handleSync(providerId: string) {
    try {
      const provider = await syncProviderModels(providerId);
      setProviders((current) =>
        current.map((entry) => (entry.id === provider.id ? provider : entry)),
      );
      setFeedback(`Synced models for ${provider.displayName}.`);
      setError(null);
    } catch (syncError) {
      setError(
        syncError instanceof Error
          ? syncError.message
          : "Failed to sync provider models.",
      );
    }
  }

  return (
    <div className="stack-gap">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">Providers</p>
            <h2>Register model backends and sync the visible model catalog.</h2>
          </div>
          <p>
            The UI uses provider endpoints to register accounts, test
            connectivity, and refresh the model list shown across the workspace.
          </p>
          {feedback ? <p className="status-pill tone-mint">{feedback}</p> : null}
          {error ? <p className="error-copy">{error}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{providers.length}</strong>
              <span>configured providers</span>
            </article>
          </div>
        </div>
      </section>

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Register</p>
            <h2>Mainstream model accounts</h2>
          </div>
        </div>
        <form className="form-grid" onSubmit={handleCreate}>
          <label>
            Provider type
            <select
              className="text-input"
              onChange={(event) => setProviderType(event.target.value as ProviderType)}
              value={providerType}
            >
              {PROVIDER_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
          <label>
            Display name
            <input
              className="text-input"
              onChange={(event) => setDisplayName(event.target.value)}
              value={displayName}
            />
          </label>
          <label>
            Base URL
            <input
              className="text-input"
              onChange={(event) => setBaseUrl(event.target.value)}
              placeholder="Optional override"
              value={baseUrl}
            />
          </label>
          <button className="primary-button" type="submit">
            Register provider
          </button>
        </form>
      </section>

      <section className="catalog-grid">
        {providers.map((provider) => (
          <article className="catalog-card" key={provider.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{provider.providerType}</p>
                <h3>{provider.displayName}</h3>
              </div>
              <span className="status-pill">{humanizeLabel(provider.status)}</span>
            </div>
            <p className="hint-copy">{provider.baseUrl ?? "No base URL override"}</p>
            <div className="button-row">
              <button
                className="ghost-button"
                onClick={() => void handleTest(provider.id)}
                type="button"
              >
                Test
              </button>
              <button
                className="ghost-button"
                onClick={() => void handleSync(provider.id)}
                type="button"
              >
                Sync models
              </button>
            </div>
            <div className="chip-row">
              {provider.models.map((model) => (
                <span className="chip" key={model.id}>
                  {model.modelName}
                </span>
              ))}
            </div>
          </article>
        ))}
      </section>
    </div>
  );
}
