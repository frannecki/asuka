"use client";

import { useEffect, useState } from "react";

import {
  createProvider,
  listProviders,
  syncProviderModels,
  testProvider,
} from "@/lib/api";
import type { ProviderAccountRecord, ProviderType } from "@/lib/types";

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

  useEffect(() => {
    let cancelled = false;

    void listProviders().then((nextProviders) => {
      if (!cancelled) {
        setProviders(nextProviders);
      }
    });

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const created = await createProvider({
      providerType,
      displayName,
      baseUrl: baseUrl || null,
    });

    setProviders((current) => [created, ...current]);
    setFeedback(`Created provider account for ${created.displayName}.`);
  }

  async function handleTest(providerId: string) {
    const result = await testProvider(providerId);
    setFeedback(result.message);
  }

  async function handleSync(providerId: string) {
    const provider = await syncProviderModels(providerId);
    setProviders((current) =>
      current.map((entry) => (entry.id === provider.id ? provider : entry)),
    );
    setFeedback(`Synced models for ${provider.displayName}.`);
  }

  return (
    <div className="stack-gap">
      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Providers</p>
            <h2>Mainstream model accounts</h2>
          </div>
        </div>
        <form className="form-grid" onSubmit={handleCreate}>
          <label>
            Provider type
            <select
              className="text-input"
              onChange={(event) =>
                setProviderType(event.target.value as ProviderType)
              }
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
        {feedback ? <p className="hint-copy">{feedback}</p> : null}
      </section>

      <section className="grid-cards">
        {providers.map((provider) => (
          <article className="panel stack-gap" key={provider.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{provider.providerType}</p>
                <h2>{provider.displayName}</h2>
              </div>
              <span className="status-pill">{provider.status}</span>
            </div>
            <p className="hint-copy">{provider.baseUrl ?? "No base URL"}</p>
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
