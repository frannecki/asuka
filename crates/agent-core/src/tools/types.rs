use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::error::CoreResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolDescriptor {
    pub id: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolResult {
    pub ok: bool,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolContext {
    pub workspace_root: PathBuf,
    pub session_id: Uuid,
}

#[async_trait]
pub(crate) trait Tool: Send + Sync {
    fn descriptor(&self) -> ToolDescriptor;

    async fn execute(&self, ctx: ToolContext, input: Value) -> CoreResult<ToolResult>;
}
