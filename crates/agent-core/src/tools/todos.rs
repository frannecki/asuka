use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::fs;

use crate::{
    error::{CoreError, CoreResult},
    tools::{
        files::workspace_file_artifact,
        types::{Tool, ToolContext, ToolDescriptor, ToolResult},
    },
};

use super::files::{display_relative_path, resolve_workspace_path};

pub(crate) struct WriteTodosTool;
pub(crate) struct ReadTodosTool;

#[async_trait]
impl Tool for WriteTodosTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "write_todos".to_string(),
            description: "Write a markdown todo list for the current session.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["items"],
                "properties": {
                    "path": { "type": "string", "description": "Optional workspace-relative todo file path. Defaults to .asuka/todos/<session>.md." },
                    "items": {
                        "type": "array",
                        "description": "Todo items to write.",
                        "items": {
                            "oneOf": [
                                { "type": "string" },
                                {
                                    "type": "object",
                                    "properties": {
                                        "text": { "type": "string" },
                                        "done": { "type": "boolean" }
                                    }
                                }
                            ]
                        }
                    },
                    "title": { "type": "string", "description": "Optional title line." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let items = input
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| CoreError::bad_request("write_todos requires 'items'"))?;
        if items.is_empty() {
            return Err(CoreError::bad_request(
                "write_todos requires at least one item",
            ));
        }

        let title = input
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Session Todos");
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| default_todo_path(&ctx));
        let target = resolve_workspace_path(&ctx.workspace_root, &path, true)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await.map_err(|error| {
                CoreError::bad_request(format!(
                    "failed to create todo directory {}: {error}",
                    parent.display()
                ))
            })?;
        }

        let mut lines = vec![format!("# {title}"), String::new()];
        for item in items {
            match item {
                Value::String(text) => lines.push(format!("- [ ] {text}")),
                Value::Object(object) => {
                    let text = object.get("text").and_then(Value::as_str).ok_or_else(|| {
                        CoreError::bad_request("todo object items require 'text'")
                    })?;
                    let done = object.get("done").and_then(Value::as_bool).unwrap_or(false);
                    let marker = if done { "x" } else { " " };
                    lines.push(format!("- [{marker}] {text}"));
                }
                _ => {
                    return Err(CoreError::bad_request(
                        "todo items must be strings or {text, done} objects",
                    ))
                }
            }
        }

        let content = format!("{}\n", lines.join("\n"));
        fs::write(&target, &content).await.map_err(|error| {
            CoreError::bad_request(format!("failed to write todo file {}: {error}", path))
        })?;

        let relative_path = display_relative_path(&ctx.workspace_root, &target);
        Ok(ToolResult::success(json!({
            "path": display_relative_path(&ctx.workspace_root, &target),
            "content": content,
            "itemsWritten": items.len()
        }))
        .with_artifacts(vec![workspace_file_artifact(
            &relative_path,
            "session-todos.md",
            &content,
        )]))
    }
}

#[async_trait]
impl Tool for ReadTodosTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "read_todos".to_string(),
            description: "Read the markdown todo list for the current session.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Optional workspace-relative todo file path. Defaults to .asuka/todos/<session>.md." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| default_todo_path(&ctx));
        let target = resolve_workspace_path(&ctx.workspace_root, &path, false)?;
        let content = fs::read_to_string(&target).await.map_err(|error| {
            CoreError::bad_request(format!("failed to read todo file {path}: {error}"))
        })?;

        Ok(ToolResult::success(json!({
            "path": display_relative_path(&ctx.workspace_root, &target),
            "content": content
        })))
    }
}

fn default_todo_path(ctx: &ToolContext) -> String {
    format!(".asuka/todos/{}.md", ctx.session_id)
}
