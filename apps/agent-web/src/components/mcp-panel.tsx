"use client";

import { useEffect, useState } from "react";

import {
  createMcpServer,
  getMcpCapabilities,
  listMcpServers,
  testMcpServer,
} from "@/lib/api";
import type { McpServerRecord } from "@/lib/types";
import { humanizeLabel } from "@/lib/view";

export function McpPanel() {
  const [servers, setServers] = useState<McpServerRecord[]>([]);
  const [name, setName] = useState("");
  const [transport, setTransport] = useState("stdio");
  const [command, setCommand] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void listMcpServers()
      .then((nextServers) => {
        if (!cancelled) {
          setServers(nextServers);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(
            loadError instanceof Error ? loadError.message : "Failed to load MCP servers.",
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
      const created = await createMcpServer({ name, transport, command });
      setServers((current) => [created, ...current]);
      setName("");
      setTransport("stdio");
      setCommand("");
      setError(null);
    } catch (createError) {
      setError(
        createError instanceof Error
          ? createError.message
          : "Failed to create MCP server.",
      );
    }
  }

  async function handleTest(serverId: string) {
    try {
      const result = await testMcpServer(serverId);
      setFeedback(result.message);
      setError(null);
    } catch (testError) {
      setError(
        testError instanceof Error ? testError.message : "Failed to test MCP server.",
      );
    }
  }

  async function handleCapabilities(serverId: string) {
    try {
      const result = await getMcpCapabilities(serverId);
      setFeedback(`Capabilities: ${result.capabilities.join(", ")}`);
      setError(null);
    } catch (capabilityError) {
      setError(
        capabilityError instanceof Error
          ? capabilityError.message
          : "Failed to load MCP capabilities.",
      );
    }
  }

  return (
    <div className="stack-gap">
      <section className="hero-shell panel">
        <div className="hero-copy">
          <div>
            <p className="eyebrow">MCP servers</p>
            <h2>Track external tool surfaces and inspect their capabilities.</h2>
          </div>
          <p>
            The frontend registers MCP servers, probes connectivity, and pulls
            capability lists directly from the backend integration layer.
          </p>
          {feedback ? <p className="status-pill tone-mint">{feedback}</p> : null}
          {error ? <p className="error-copy">{error}</p> : null}
        </div>

        <div className="hero-art">
          <div className="hero-stat-strip">
            <article className="hero-stat">
              <strong>{servers.length}</strong>
              <span>registered servers</span>
            </article>
          </div>
        </div>
      </section>

      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Register</p>
            <h2>Server registry</h2>
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
            Transport
            <input
              className="text-input"
              onChange={(event) => setTransport(event.target.value)}
              value={transport}
            />
          </label>
          <label>
            Command
            <input
              className="text-input"
              onChange={(event) => setCommand(event.target.value)}
              value={command}
            />
          </label>
          <button className="primary-button" type="submit">
            Register MCP server
          </button>
        </form>
      </section>

      <section className="catalog-grid">
        {servers.map((server) => (
          <article className="catalog-card" key={server.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{server.transport}</p>
                <h3>{server.name}</h3>
              </div>
              <span className="status-pill">{humanizeLabel(server.status)}</span>
            </div>
            <p>{server.command}</p>
            <div className="button-row">
              <button
                className="ghost-button"
                onClick={() => void handleTest(server.id)}
                type="button"
              >
                Test
              </button>
              <button
                className="ghost-button"
                onClick={() => void handleCapabilities(server.id)}
                type="button"
              >
                Capabilities
              </button>
            </div>
          </article>
        ))}
      </section>
    </div>
  );
}
