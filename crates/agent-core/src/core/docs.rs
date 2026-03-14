use serde_json::{json, Value};

pub(crate) fn root_docs() -> Value {
    json!({
        "name": "agent-api",
        "status": "ok",
        "docs": {
            "sessions": "/api/v1/sessions",
            "providers": "/api/v1/providers",
            "skills": "/api/v1/skills",
            "subagents": "/api/v1/subagents",
            "memory": "/api/v1/memory/documents",
            "mcp": "/api/v1/mcp/servers"
        }
    })
}
