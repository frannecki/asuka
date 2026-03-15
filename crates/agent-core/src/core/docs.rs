use serde_json::{json, Value};

pub(crate) fn root_docs() -> Value {
    json!({
        "name": "agent-api",
        "status": "ok",
        "docs": {
            "sessions": "/api/v1/sessions",
            "sessionActiveRun": "/api/v1/sessions/:session_id/active-run",
            "tasks": "/api/v1/tasks",
            "taskExecution": "/api/v1/tasks/:task_id/execution",
            "runEventsHistory": "/api/v1/runs/:run_id/events/history",
            "artifacts": "/api/v1/sessions/:session_id/artifacts",
            "providers": "/api/v1/providers",
            "skills": "/api/v1/skills",
            "skillPresets": "/api/v1/skill-presets",
            "sessionSkills": "/api/v1/sessions/:session_id/skills",
            "subagents": "/api/v1/subagents",
            "memory": "/api/v1/memory/documents",
            "mcp": "/api/v1/mcp/servers",
            "workspace": "/api/v1/sessions/:session_id/workspace/tree"
        }
    })
}
