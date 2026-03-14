"use client";

import { useEffect, useState } from "react";

import {
  createMcpServer,
  getMcpCapabilities,
  listMcpServers,
  testMcpServer,
} from "@/lib/api";
import type { McpServerRecord } from "@/lib/types";

export function McpPanel() {
  const [servers, setServers] = useState<McpServerRecord[]>([]);
  const [name, setName] = useState("");
  const [transport, setTransport] = useState("stdio");
  const [command, setCommand] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);

  useEffect(() => {
    void listMcpServers().then(setServers);
  }, []);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const created = await createMcpServer({ name, transport, command });
    setServers((current) => [created, ...current]);
    setName("");
    setTransport("stdio");
    setCommand("");
  }

  async function handleTest(serverId: string) {
    const result = await testMcpServer(serverId);
    setFeedback(result.message);
  }

  async function handleCapabilities(serverId: string) {
    const result = await getMcpCapabilities(serverId);
    setFeedback(`Capabilities: ${result.capabilities.join(", ")}`);
  }

  return (
    <div className="stack-gap">
      <section className="panel stack-gap">
        <div className="panel-header">
          <div>
            <p className="eyebrow">MCP</p>
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
        {feedback ? <p className="hint-copy">{feedback}</p> : null}
      </section>

      <section className="grid-cards">
        {servers.map((server) => (
          <article className="panel stack-gap" key={server.id}>
            <div className="panel-header">
              <div>
                <p className="eyebrow">{server.transport}</p>
                <h2>{server.name}</h2>
              </div>
              <span className="status-pill">{server.status}</span>
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
