use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::fs;

use crate::{
    error::{CoreError, CoreResult},
    tools::types::{Tool, ToolContext, ToolDescriptor, ToolResult},
};

pub(crate) struct ReadFileTool;
pub(crate) struct WriteFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "read_file".to_string(),
            description: "Read a UTF-8 text file from the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative file path." },
                    "startLine": { "type": "integer", "description": "Optional 1-based starting line." },
                    "endLine": { "type": "integer", "description": "Optional 1-based ending line." },
                    "maxBytes": { "type": "integer", "description": "Optional maximum bytes to return. Defaults to 12000." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("read_file requires 'path'"))?;
        let target = resolve_workspace_path(&ctx.workspace_root, path, false)?;
        let content = fs::read_to_string(&target)
            .await
            .map_err(|error| CoreError::bad_request(format!("failed to read {path}: {error}")))?;

        let start_line = input.get("startLine").and_then(Value::as_u64).unwrap_or(1) as usize;
        let end_line = input
            .get("endLine")
            .and_then(Value::as_u64)
            .map(|value| value as usize);
        let max_bytes = clamp_usize(
            input.get("maxBytes").and_then(Value::as_u64),
            12_000,
            100_000,
        );

        let lines = content.lines().collect::<Vec<_>>();
        let start_index = start_line.saturating_sub(1).min(lines.len());
        let end_index = end_line.unwrap_or(lines.len()).min(lines.len());
        let mut selected = if start_index >= end_index {
            String::new()
        } else {
            lines[start_index..end_index].join("\n")
        };
        let mut truncated = false;
        if selected.len() > max_bytes {
            selected.truncate(max_bytes);
            truncated = true;
        }

        Ok(ToolResult {
            ok: true,
            payload: json!({
                "path": display_relative_path(&ctx.workspace_root, &target),
                "content": selected,
                "lineStart": start_line,
                "lineEnd": end_index,
                "truncated": truncated
            }),
        })
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "write_file".to_string(),
            description: "Write a UTF-8 text file in the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative file path." },
                    "content": { "type": "string", "description": "Text content to write." },
                    "mode": { "type": "string", "enum": ["overwrite", "append"], "description": "Write mode. Defaults to overwrite." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let path = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("write_file requires 'path'"))?;
        let content = input
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("write_file requires 'content'"))?;
        let mode = input
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("overwrite");
        if mode != "overwrite" && mode != "append" {
            return Err(CoreError::bad_request(
                "write_file mode must be overwrite or append",
            ));
        }

        let target = resolve_workspace_path(&ctx.workspace_root, path, true)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await.map_err(|error| {
                CoreError::bad_request(format!(
                    "failed to create parent directory for {path}: {error}"
                ))
            })?;
        }

        match mode {
            "append" => {
                use tokio::io::AsyncWriteExt;

                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&target)
                    .await
                    .map_err(|error| {
                        CoreError::bad_request(format!("failed to open {path} for append: {error}"))
                    })?;
                file.write_all(content.as_bytes()).await.map_err(|error| {
                    CoreError::bad_request(format!("failed to append to {path}: {error}"))
                })?;
            }
            _ => {
                fs::write(&target, content).await.map_err(|error| {
                    CoreError::bad_request(format!("failed to write {path}: {error}"))
                })?;
            }
        }

        Ok(ToolResult {
            ok: true,
            payload: json!({
                "path": display_relative_path(&ctx.workspace_root, &target),
                "bytesWritten": content.len(),
                "mode": mode
            }),
        })
    }
}

pub(crate) fn clamp_usize(value: Option<u64>, default: usize, max: usize) -> usize {
    value
        .and_then(|value| usize::try_from(value).ok())
        .map(|value| value.min(max))
        .unwrap_or(default)
}

pub(crate) fn resolve_workspace_path(
    workspace_root: &Path,
    candidate: &str,
    allow_missing: bool,
) -> CoreResult<PathBuf> {
    let workspace_root = std::fs::canonicalize(workspace_root).map_err(|error| {
        CoreError::new(
            500,
            format!(
                "failed to resolve workspace root {}: {error}",
                workspace_root.display()
            ),
        )
    })?;
    let joined = if Path::new(candidate).is_absolute() {
        PathBuf::from(candidate)
    } else {
        workspace_root.join(candidate)
    };
    let normalized = normalize_path(&joined);
    if allow_missing {
        if !normalized.starts_with(&workspace_root) {
            return Err(CoreError::bad_request(format!(
                "path {} is outside the workspace root {}",
                normalized.display(),
                workspace_root.display()
            )));
        }

        return Ok(normalized);
    }

    let resolved = if normalized.exists() {
        std::fs::canonicalize(&normalized).map_err(|error| {
            CoreError::bad_request(format!(
                "failed to resolve path {}: {error}",
                normalized.display()
            ))
        })?
    } else {
        return Err(CoreError::not_found("path"));
    };

    if !resolved.starts_with(&workspace_root) {
        return Err(CoreError::bad_request(format!(
            "path {} is outside the workspace root {}",
            resolved.display(),
            workspace_root.display()
        )));
    }

    Ok(resolved)
}

pub(crate) fn display_relative_path(workspace_root: &Path, target: &Path) -> String {
    target
        .strip_prefix(workspace_root)
        .ok()
        .map(|value| value.display().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| target.display().to_string())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::resolve_workspace_path;

    #[test]
    fn resolve_workspace_path_allows_nested_missing_paths_inside_workspace() {
        let workspace_root =
            std::env::temp_dir().join(format!("asuka-tools-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace_root).expect("create temp workspace");

        let resolved = resolve_workspace_path(&workspace_root, ".asuka/nested/file.txt", true)
            .expect("resolve nested path");

        assert!(resolved.ends_with(".asuka/nested/file.txt"));
        assert!(
            resolved.starts_with(std::fs::canonicalize(&workspace_root).expect("canonical root"))
        );
    }
}
