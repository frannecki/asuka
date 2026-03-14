import Link from "next/link";

export default function Home() {
  return (
    <div className="stack-gap">
      <section className="hero panel">
        <div>
          <p className="eyebrow">Prototype status</p>
          <h2>Backend and frontend scaffolds are now wired together.</h2>
        </div>
        <p>
          This first cut includes a Rust API with seeded provider, skill,
          subagent, and MCP registries plus a Next.js operator console with
          chat streaming.
        </p>
      </section>

      <section className="grid-cards">
        <Link className="panel feature-card" href="/chat">
          <p className="eyebrow">Chat</p>
          <h2>Streamed agent runs</h2>
          <p>
            Sessions, transcript state, and live tool or subagent activity in
            one view.
          </p>
        </Link>
        <Link className="panel feature-card" href="/memory">
          <p className="eyebrow">Memory</p>
          <h2>RAG document surface</h2>
          <p>
            Add long-term memory documents, inspect chunking, and run lexical
            retrieval over the stored corpus.
          </p>
        </Link>
        <Link className="panel feature-card" href="/skills">
          <p className="eyebrow">Skills</p>
          <h2>Capability registry</h2>
          <p>Register reusable skill bundles and inspect seeded examples.</p>
        </Link>
        <Link className="panel feature-card" href="/subagents">
          <p className="eyebrow">Subagents</p>
          <h2>Delegation surfaces</h2>
          <p>Define specialist workers with bounded scopes and step budgets.</p>
        </Link>
        <Link className="panel feature-card" href="/settings/providers">
          <p className="eyebrow">Providers</p>
          <h2>Mainstream LLM accounts</h2>
          <p>Manage OpenAI, Anthropic, Gemini, Azure OpenAI, and more.</p>
        </Link>
        <Link className="panel feature-card" href="/settings/mcp">
          <p className="eyebrow">MCP</p>
          <h2>Server registration</h2>
          <p>Track MCP endpoints and inspect their exposed capabilities.</p>
        </Link>
      </section>
    </div>
  );
}
