use std::{path::PathBuf, time::UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::fs;

use crate::{
    error::{CoreError, CoreResult},
    tools::types::{Tool, ToolContext, ToolDescriptor, ToolResult},
};

use super::files::{clamp_usize, resolve_workspace_path};

pub(crate) struct ListTool;

#[async_trait]
impl Tool for ListTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "list".to_string(),
            description: "List files and directories under a workspace-relative path.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative directory path. Defaults to '.'." },
                    "maxEntries": { "type": "integer", "description": "Maximum number of entries to return. Defaults to 200." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let path = input.get("path").and_then(Value::as_str).unwrap_or(".");
        let target = resolve_workspace_path(&ctx.workspace_root, path, false)?;
        let metadata = fs::metadata(&target)
            .await
            .map_err(|error| CoreError::bad_request(format!("failed to stat {path}: {error}")))?;
        if !metadata.is_dir() {
            return Err(CoreError::bad_request(format!("{path} is not a directory")));
        }

        let max_entries = clamp_usize(input.get("maxEntries").and_then(Value::as_u64), 200, 500);
        let mut entries = fs::read_dir(&target).await.map_err(|error| {
            CoreError::bad_request(format!("failed to read directory {path}: {error}"))
        })?;
        let mut items = Vec::new();
        let mut truncated = false;

        while let Some(entry) = entries.next_entry().await.map_err(|error| {
            CoreError::bad_request(format!("failed to iterate directory {path}: {error}"))
        })? {
            if items.len() >= max_entries {
                truncated = true;
                break;
            }

            let entry_path = entry.path();
            let metadata = entry.metadata().await.map_err(|error| {
                CoreError::bad_request(format!(
                    "failed to read metadata for {}: {error}",
                    entry_path.display()
                ))
            })?;
            let kind = if metadata.is_dir() {
                "dir"
            } else if metadata.is_file() {
                "file"
            } else {
                "other"
            };
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_secs());

            items.push(json!({
                "name": entry.file_name().to_string_lossy().to_string(),
                "path": to_relative(&ctx.workspace_root, &entry_path),
                "kind": kind,
                "size": metadata.len(),
                "modifiedUnix": modified
            }));
        }

        items.sort_by(|left, right| {
            left.get("path")
                .and_then(Value::as_str)
                .cmp(&right.get("path").and_then(Value::as_str))
        });

        Ok(ToolResult {
            ok: true,
            payload: json!({
                "path": to_relative(&ctx.workspace_root, &target),
                "entries": items,
                "truncated": truncated
            }),
        })
    }
}

fn to_relative(workspace_root: &PathBuf, target: &PathBuf) -> String {
    target
        .strip_prefix(workspace_root)
        .ok()
        .map(|value| value.display().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| ".".to_string())
}
