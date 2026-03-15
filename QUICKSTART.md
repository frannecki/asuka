# Quickstart

This guide shows how to run Asuka locally.

## Prerequisites

- Rust toolchain
- Node.js 20+ and `npm`
- Python 3
- At least one provider API key:
  - `OPENROUTER_API_KEY`, or
  - `MOONSHOT_API_KEY`

Optional:

- ChromaDB, if you want semantic memory retrieval instead of lexical-only fallback

## 1. Install Frontend Dependencies

```bash
cd apps/agent-web
npm install
cd ../..
```

## 2. Configure Environment

At minimum, export one live provider key:

```bash
export OPENROUTER_API_KEY=...
```

or:

```bash
export MOONSHOT_API_KEY=...
```

Useful optional variables:

```bash
export SQLITE_PATH=./data/asuka.sqlite3
export ASUKA_WORKSPACE_ROOT=$PWD
export MODELS_CONFIG_PATH=$PWD/config/models.toml
export PORT=4000
```

For Chroma, either run it locally or explicitly disable it:

```bash
export CHROMA_DISABLED=1
```

If you want semantic retrieval, do not set `CHROMA_DISABLED`.

## 3. Start ChromaDB (Optional)

If Chroma is installed locally:

```bash
chroma run --path ./data/chroma --host 127.0.0.1 --port 8000
```

Default Asuka Chroma settings:

- `CHROMA_URL=http://127.0.0.1:8000`
- `CHROMA_TENANT=default_tenant`
- `CHROMA_DATABASE=default_database`
- `CHROMA_COLLECTION=asuka-memory`

If Chroma is unavailable, Asuka still runs and falls back to lexical search.

## 4. Start the Backend

From the repo root:

```bash
cargo run --bin agent-api
```

The API listens on:

```text
http://127.0.0.1:4000
```

## 5. Start the Frontend

In another terminal:

```bash
cd apps/agent-web
npm run dev -- --hostname 127.0.0.1 --port 3000
```

Open:

```text
http://127.0.0.1:3000/dashboard
```

## 6. First Local Flow

1. Open the dashboard
2. Create a new session
3. Send a message in the chat workspace
4. Watch the live stream update in the chat inspector
5. Open the session execution view to inspect run steps, tools, and lineage
6. Open the artifacts view to preview Markdown, JSON, and generated HTML reports

## Smoke-Test Commands

Backend checks:

```bash
cargo check
cargo test
```

Frontend checks:

```bash
cd apps/agent-web
npm test
npm run build
```

## Common Paths

- SQLite DB: `./data/asuka.sqlite3`
- Workspace artifacts: `./.asuka/workspaces/`
- Model config: `./config/models.toml`

## Notes

- The project defaults to SQLite storage.
- Provider/model metadata is loaded from `config/models.toml`.
- Workspace artifacts and run traces are stored locally and rendered directly in the frontend.
- The system is optimized for local, single-user operation.
