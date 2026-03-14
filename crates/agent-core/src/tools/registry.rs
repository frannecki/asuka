use std::{collections::HashMap, path::PathBuf, sync::Arc};

use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::{CoreError, CoreResult},
    tools::{
        files::{ReadFileTool, WriteFileTool},
        fs_ops::{DeletePathTool, MkdirTool, MovePathTool, StatTool},
        glob::GlobTool,
        list::ListTool,
        ripgrep::RipgrepTool,
        todos::{ReadTodosTool, WriteTodosTool},
        types::{Tool, ToolContext, ToolDescriptor, ToolResult},
    },
};

#[derive(Clone)]
pub(crate) struct ToolRegistry {
    workspace_root: PathBuf,
    tools: Arc<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub(crate) fn new(workspace_root: PathBuf) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        for tool in [
            Arc::new(StatTool) as Arc<dyn Tool>,
            Arc::new(MkdirTool) as Arc<dyn Tool>,
            Arc::new(MovePathTool) as Arc<dyn Tool>,
            Arc::new(DeletePathTool) as Arc<dyn Tool>,
            Arc::new(GlobTool) as Arc<dyn Tool>,
            Arc::new(ListTool) as Arc<dyn Tool>,
            Arc::new(ReadFileTool) as Arc<dyn Tool>,
            Arc::new(WriteFileTool) as Arc<dyn Tool>,
            Arc::new(RipgrepTool) as Arc<dyn Tool>,
            Arc::new(WriteTodosTool) as Arc<dyn Tool>,
            Arc::new(ReadTodosTool) as Arc<dyn Tool>,
        ] {
            tools.insert(tool.descriptor().id.clone(), tool);
        }

        Self {
            workspace_root,
            tools: Arc::new(tools),
        }
    }

    pub(crate) fn descriptors(&self) -> Vec<ToolDescriptor> {
        let mut descriptors = self
            .tools
            .values()
            .map(|tool| tool.descriptor())
            .collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        descriptors
    }

    pub(crate) async fn execute(
        &self,
        session_id: Uuid,
        tool_name: &str,
        arguments: Value,
    ) -> CoreResult<ToolResult> {
        let Some(tool) = self.tools.get(tool_name) else {
            return Err(CoreError::bad_request(format!(
                "unknown tool '{tool_name}'"
            )));
        };

        tool.execute(
            ToolContext {
                workspace_root: self.workspace_root.clone(),
                session_id,
            },
            arguments,
        )
        .await
    }
}
