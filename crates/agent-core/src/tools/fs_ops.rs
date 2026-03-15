use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::fs;

use crate::{
    error::{CoreError, CoreResult},
    tools::types::{Tool, ToolContext, ToolDescriptor, ToolResult},
};

use super::files::{display_relative_path, resolve_workspace_path};

pub(crate) struct StatTool;
pub(crate) struct MkdirTool;
pub(crate) struct MovePathTool;
pub(crate) struct DeletePathTool;

#[async_trait]
impl Tool for StatTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "stat".to_string(),
            description: "Read metadata about a file or directory in the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative path to inspect." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("stat requires 'path'"))?;
        let target = resolve_workspace_path(&ctx.workspace_root, path, false)?;
        let metadata = fs::metadata(&target)
            .await
            .map_err(|error| CoreError::bad_request(format!("failed to stat {path}: {error}")))?;
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

        Ok(ToolResult::success(json!({
            "path": display_relative_path(&ctx.workspace_root, &target),
            "kind": kind,
            "size": metadata.len(),
            "readonly": metadata.permissions().readonly(),
            "modifiedUnix": modified
        })))
    }
}

#[async_trait]
impl Tool for MkdirTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "mkdir".to_string(),
            description: "Create a directory in the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative directory path to create." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("mkdir requires 'path'"))?;
        let target = resolve_workspace_path(&ctx.workspace_root, path, true)?;
        fs::create_dir_all(&target)
            .await
            .map_err(|error| CoreError::bad_request(format!("failed to create {path}: {error}")))?;

        Ok(ToolResult::success(json!({
            "path": display_relative_path(&ctx.workspace_root, &target),
            "created": true
        })))
    }
}

#[async_trait]
impl Tool for MovePathTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "move_path".to_string(),
            description: "Move or rename a file or directory inside the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["from", "to"],
                "properties": {
                    "from": { "type": "string", "description": "Workspace-relative source path." },
                    "to": { "type": "string", "description": "Workspace-relative destination path." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let from = input
            .get("from")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("move_path requires 'from'"))?;
        let to = input
            .get("to")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("move_path requires 'to'"))?;
        let source = resolve_workspace_path(&ctx.workspace_root, from, false)?;
        let destination = resolve_workspace_path(&ctx.workspace_root, to, true)?;

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await.map_err(|error| {
                CoreError::bad_request(format!(
                    "failed to create destination parent for {to}: {error}"
                ))
            })?;
        }

        fs::rename(&source, &destination).await.map_err(|error| {
            CoreError::bad_request(format!("failed to move {from} to {to}: {error}"))
        })?;

        Ok(ToolResult::success(json!({
            "from": display_relative_path(&ctx.workspace_root, &source),
            "to": display_relative_path(&ctx.workspace_root, &destination)
        })))
    }
}

#[async_trait]
impl Tool for DeletePathTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "delete_path".to_string(),
            description: "Delete a file or directory inside the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative path to delete." },
                    "recursive": { "type": "boolean", "description": "Required to delete directories recursively." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("delete_path requires 'path'"))?;
        let recursive = input
            .get("recursive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let target = resolve_workspace_path(&ctx.workspace_root, path, false)?;
        let metadata = fs::metadata(&target)
            .await
            .map_err(|error| CoreError::bad_request(format!("failed to stat {path}: {error}")))?;

        if metadata.is_dir() {
            if !recursive {
                return Err(CoreError::bad_request(
                    "delete_path requires recursive=true for directories",
                ));
            }
            fs::remove_dir_all(&target).await.map_err(|error| {
                CoreError::bad_request(format!("failed to delete directory {path}: {error}"))
            })?;
        } else {
            fs::remove_file(&target).await.map_err(|error| {
                CoreError::bad_request(format!("failed to delete file {path}: {error}"))
            })?;
        }

        Ok(ToolResult::success(json!({
            "path": display_relative_path(&ctx.workspace_root, &target),
            "deleted": true
        })))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::fs;
    use uuid::Uuid;

    use super::{DeletePathTool, MkdirTool, MovePathTool, StatTool};
    use crate::tools::types::{Tool, ToolContext};

    fn temp_context() -> ToolContext {
        let root = std::env::temp_dir().join(format!("asuka-fs-ops-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        ToolContext {
            workspace_root: root,
            session_id: Uuid::new_v4(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mkdir_and_stat_create_and_report_directory() {
        let ctx = temp_context();
        MkdirTool
            .execute(ctx.clone(), json!({ "path": "tmp/nested" }))
            .await
            .expect("mkdir");

        let result = StatTool
            .execute(ctx.clone(), json!({ "path": "tmp/nested" }))
            .await
            .expect("stat");
        assert_eq!(result.payload["kind"], "dir");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn move_and_delete_path_update_workspace_entries() {
        let ctx = temp_context();
        let original = ctx.workspace_root.join("from.txt");
        fs::write(&original, "hello").await.expect("seed file");

        MovePathTool
            .execute(
                ctx.clone(),
                json!({ "from": "from.txt", "to": "subdir/to.txt" }),
            )
            .await
            .expect("move path");
        assert!(!original.exists());
        assert!(ctx.workspace_root.join("subdir/to.txt").exists());

        DeletePathTool
            .execute(ctx.clone(), json!({ "path": "subdir/to.txt" }))
            .await
            .expect("delete file");
        assert!(!ctx.workspace_root.join("subdir/to.txt").exists());
    }
}
