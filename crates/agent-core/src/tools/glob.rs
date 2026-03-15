use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{
    error::{CoreError, CoreResult},
    tools::types::{Tool, ToolContext, ToolDescriptor, ToolResult},
};

use super::files::{clamp_usize, display_relative_path};

pub(crate) struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: "glob".to_string(),
            description: "Match workspace files using a glob pattern.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": { "type": "string", "description": "Workspace-relative glob pattern, e.g. crates/**/*.rs" },
                    "maxResults": { "type": "integer", "description": "Maximum number of matches to return. Defaults to 200." }
                }
            }),
        }
    }

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult> {
        let pattern = input
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::bad_request("glob requires 'pattern'"))?;
        let max_results = clamp_usize(input.get("maxResults").and_then(Value::as_u64), 200, 1000);
        let absolute_pattern = ctx.workspace_root.join(pattern);
        let pattern_string = absolute_pattern.to_string_lossy().to_string();

        let mut paths = Vec::new();
        let mut truncated = false;
        for entry in glob::glob(&pattern_string)
            .map_err(|error| CoreError::bad_request(format!("invalid glob pattern: {error}")))?
        {
            let path = entry.map_err(|error| {
                CoreError::bad_request(format!("failed while expanding glob pattern: {error}"))
            })?;
            if !path.starts_with(&ctx.workspace_root) {
                continue;
            }
            if paths.len() >= max_results {
                truncated = true;
                break;
            }
            paths.push(display_relative_path(&ctx.workspace_root, &path));
        }
        paths.sort();

        Ok(ToolResult::success(json!({
            "pattern": pattern,
            "matches": paths,
            "truncated": truncated
        })))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::fs;
    use uuid::Uuid;

    use super::GlobTool;
    use crate::tools::types::{Tool, ToolContext};

    #[tokio::test(flavor = "current_thread")]
    async fn glob_matches_workspace_files() {
        let root = std::env::temp_dir().join(format!("asuka-glob-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("nested")).expect("create temp root");
        fs::write(root.join("a.txt"), "a").await.expect("write a");
        fs::write(root.join("nested/b.txt"), "b")
            .await
            .expect("write b");

        let result = GlobTool
            .execute(
                ToolContext {
                    workspace_root: root,
                    session_id: Uuid::new_v4(),
                },
                json!({ "pattern": "**/*.txt" }),
            )
            .await
            .expect("glob result");

        let matches = result.payload["matches"].as_array().expect("matches array");
        assert_eq!(matches.len(), 2);
    }
}
