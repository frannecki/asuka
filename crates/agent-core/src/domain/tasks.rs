use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{
    ArtifactRecord, PlanStatus, PlanStepKind, PlanStepStatus, RunRecord, RunStepStatus, TaskStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub title: String,
    pub goal: String,
    pub status: TaskStatus,
    pub kind: String,
    pub origin_message_id: Uuid,
    pub current_plan_id: Option<Uuid>,
    pub latest_run_id: Option<Uuid>,
    pub summary: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRecord {
    pub id: Uuid,
    pub task_id: Uuid,
    pub version: u32,
    pub status: PlanStatus,
    pub source: String,
    pub planning_model: Option<String>,
    pub created_at: DateTime<Utc>,
    pub superseded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepRecord {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub ordinal: u32,
    pub title: String,
    pub description: String,
    pub kind: PlanStepKind,
    pub status: PlanStepStatus,
    pub depends_on: Vec<Uuid>,
    pub skill_id: Option<Uuid>,
    pub subagent_id: Option<Uuid>,
    pub expected_outputs: Vec<String>,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDetail {
    pub plan: PlanRecord,
    pub steps: Vec<PlanStepRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStepRecord {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub plan_step_id: Option<Uuid>,
    pub sequence: u32,
    pub kind: PlanStepKind,
    pub title: String,
    pub status: RunStepStatus,
    pub input_summary: String,
    pub output_summary: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInvocationRecord {
    pub id: Uuid,
    pub run_step_id: Uuid,
    pub run_id: Uuid,
    pub tool_name: String,
    pub tool_source: String,
    pub arguments_json: Value,
    pub result_json: Value,
    pub ok: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionTimelineGroup {
    pub id: String,
    pub run: RunRecord,
    pub run_steps: Vec<RunStepRecord>,
    pub tool_invocations: Vec<ToolInvocationRecord>,
    pub artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactGroupRecord {
    pub id: String,
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub title: String,
    pub summary: String,
    pub primary_artifact_id: Option<Uuid>,
    pub artifact_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LineageNodeKind {
    Task,
    Run,
    RunStep,
    ToolInvocation,
    Artifact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageNodeRecord {
    pub id: String,
    pub kind: LineageNodeKind,
    pub label: String,
    pub status: Option<String>,
    pub ref_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageEdgeRecord {
    pub from: String,
    pub to: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionDetail {
    pub task: TaskRecord,
    pub plan_detail: Option<PlanDetail>,
    pub runs: Vec<RunRecord>,
    pub timeline_groups: Vec<ExecutionTimelineGroup>,
    pub artifact_groups: Vec<ArtifactGroupRecord>,
    pub lineage_nodes: Vec<LineageNodeRecord>,
    pub lineage_edges: Vec<LineageEdgeRecord>,
}
