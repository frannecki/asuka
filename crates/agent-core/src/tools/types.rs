use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    domain::{ArtifactKind, ArtifactRenderMode},
    error::CoreResult,
};

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
    #[serde(skip_serializing)]
    pub artifacts: Vec<ToolArtifact>,
}

impl ToolResult {
    pub(crate) fn success(payload: Value) -> Self {
        Self {
            ok: true,
            payload,
            artifacts: Vec::new(),
        }
    }

    pub(crate) fn with_artifacts(mut self, artifacts: Vec<ToolArtifact>) -> Self {
        self.artifacts = artifacts;
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolArtifact {
    pub relative_path: String,
    pub display_name: String,
    pub description: String,
    pub kind: ArtifactKind,
    pub render_mode: ArtifactRenderMode,
    pub media_type: String,
    pub content: ToolArtifactContent,
}

impl ToolArtifact {
    pub(crate) fn utf8(
        relative_path: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        kind: ArtifactKind,
        render_mode: ArtifactRenderMode,
        media_type: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            display_name: display_name.into(),
            description: description.into(),
            kind,
            render_mode,
            media_type: media_type.into(),
            content: ToolArtifactContent::Utf8(content.into()),
        }
    }

    pub(crate) fn size_bytes(&self) -> u64 {
        match &self.content {
            ToolArtifactContent::Utf8(value) => value.len() as u64,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ToolArtifactContent {
    Utf8(String),
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
