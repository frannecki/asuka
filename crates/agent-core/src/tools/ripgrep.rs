use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::{
    error::{CoreError, CoreResult},
    tools::types::{Tool, ToolContext, ToolDescriptor, ToolResult},
};

use super::files::{clamp_usize, display_relative_path, resolve_workspace_path};

pub(crate) struct RipgrepTool;

#[async_trait]
impl Tool for RipgrepTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "ripgrep".to_string(),
            description: "Search text in the workspace using ripgrep.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": { "type": "string", "description": "Pattern to search for." },
                    "path": { "type": "string", "description": "Workspace-relative search root. Defaults to '.'." },
                    "maxResults": { "type": "integer", "description": "Maximum number of matches to return. Defaults to 100." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let pattern = input
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("ripgrep requires 'pattern'"))?;
        let path = input.get("path").and_then(Value::as_str).unwrap_or(".");
        let target = resolve_workspace_path(&ctx.workspace_root, path, false)?;
        let max_results = clamp_usize(input.get("maxResults").and_then(Value::as_u64), 100, 500);

        let output = Command::new("rg")
            .arg("--json")
            .arg("--line-number")
            .arg("--color")
            .arg("never")
            .arg(pattern)
            .arg(&target)
            .output()
            .await
            .map_err(|error| CoreError::bad_request(format!("failed to run ripgrep: {error}")))?;

        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(CoreError::bad_request(format!(
                "ripgrep failed with status {}: {stderr}",
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut matches = Vec::new();
        let mut truncated = false;

        for line in stdout.lines() {
            if matches.len() >= max_results {
                truncated = true;
                break;
            }

            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if value.get("type").and_then(Value::as_str) != Some("match") {
                continue;
            }

            let data = value.get("data").cloned().unwrap_or_else(|| json!({}));
            let path_text = data
                .get("path")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let line_number = data.get("line_number").and_then(Value::as_u64).unwrap_or(0);
            let text = data
                .get("lines")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim_end()
                .to_string();

            matches.push(json!({
                "path": display_relative_path(&ctx.workspace_root, &target.join(path_text).strip_prefix(&target).unwrap_or_else(|_| std::path::Path::new(path_text)).to_path_buf()),
                "line": line_number,
                "text": text
            }));
        }

        Ok(ToolResult::success(json!({
            "pattern": pattern,
            "path": display_relative_path(&ctx.workspace_root, &target),
            "matches": matches,
            "truncated": truncated
        })))
    }
}
