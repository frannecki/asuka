use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{
        ArtifactGroupRecord, ArtifactKind, ArtifactProducerKind, ArtifactRecord,
        ExecutionTimelineGroup, LineageEdgeRecord, LineageNodeKind, LineageNodeRecord, PlanDetail,
        PlanStepKind, RunRecord, RunStepRecord, TaskExecutionDetail, TaskRecord,
    },
    error::CoreResult,
};

impl AgentCore {
    pub async fn list_tasks(&self, session_id: Option<Uuid>) -> CoreResult<Vec<TaskRecord>> {
        self.store.list_tasks(session_id).await
    }

    pub async fn get_task(&self, task_id: Uuid) -> CoreResult<TaskRecord> {
        self.store.get_task(task_id).await
    }

    pub async fn get_task_plan(&self, task_id: Uuid) -> CoreResult<PlanDetail> {
        self.store.get_task_plan(task_id).await
    }

    pub async fn get_task_execution(&self, task_id: Uuid) -> CoreResult<TaskExecutionDetail> {
        let task = self.get_task(task_id).await?;
        let plan_detail = self.get_task_plan(task_id).await.ok();
        let runs = self.store.list_task_runs(task_id).await?;
        let artifacts = self.store.list_task_artifacts(task_id).await?;

        let mut timeline_groups = Vec::new();
        let mut artifact_groups = Vec::new();
        let mut lineage_nodes = vec![LineageNodeRecord {
            id: task_node_id(task.id),
            kind: LineageNodeKind::Task,
            label: task.title.clone(),
            status: Some(enum_label(&task.status)),
            ref_id: Some(task.id),
        }];
        let mut lineage_edges = Vec::new();

        for run in &runs {
            let run_steps = self.list_run_steps(run.id).await?;
            let tool_invocations = self.list_tool_invocations(run.id).await?;
            let run_artifacts = artifacts_for_run(&artifacts, run.id);
            let run_node_id = run_node_id(run.id);

            lineage_nodes.push(LineageNodeRecord {
                id: run_node_id.clone(),
                kind: LineageNodeKind::Run,
                label: format!("Run {}", short_id(run.id)),
                status: Some(enum_label(&run.status)),
                ref_id: Some(run.id),
            });
            lineage_edges.push(LineageEdgeRecord {
                from: task_node_id(task.id),
                to: run_node_id.clone(),
                relation: "executes".to_string(),
            });

            for step in &run_steps {
                let step_node_id = run_step_node_id(step.id);
                lineage_nodes.push(LineageNodeRecord {
                    id: step_node_id.clone(),
                    kind: LineageNodeKind::RunStep,
                    label: step.title.clone(),
                    status: Some(enum_label(&step.status)),
                    ref_id: Some(step.id),
                });
                lineage_edges.push(LineageEdgeRecord {
                    from: run_node_id.clone(),
                    to: step_node_id.clone(),
                    relation: "contains".to_string(),
                });

                for invocation in tool_invocations
                    .iter()
                    .filter(|invocation| invocation.run_step_id == step.id)
                {
                    let invocation_node_id = tool_node_id(invocation.id);
                    lineage_nodes.push(LineageNodeRecord {
                        id: invocation_node_id.clone(),
                        kind: LineageNodeKind::ToolInvocation,
                        label: invocation.tool_name.clone(),
                        status: Some(if invocation.ok {
                            "ok".to_string()
                        } else {
                            "error".to_string()
                        }),
                        ref_id: Some(invocation.id),
                    });
                    lineage_edges.push(LineageEdgeRecord {
                        from: step_node_id.clone(),
                        to: invocation_node_id,
                        relation: "calls".to_string(),
                    });
                }
            }

            for artifact in &run_artifacts {
                let artifact_node_id = artifact_node_id(artifact.id);
                lineage_nodes.push(LineageNodeRecord {
                    id: artifact_node_id.clone(),
                    kind: LineageNodeKind::Artifact,
                    label: artifact.display_name.clone(),
                    status: Some(enum_label(&artifact.kind)),
                    ref_id: Some(artifact.id),
                });
                lineage_edges.push(LineageEdgeRecord {
                    from: producer_node_for_artifact(run, &run_steps, artifact),
                    to: artifact_node_id,
                    relation: "produces".to_string(),
                });
            }

            timeline_groups.push(ExecutionTimelineGroup {
                id: format!("run-{}", run.id),
                run: run.clone(),
                run_steps: run_steps.clone(),
                tool_invocations: tool_invocations.clone(),
                artifacts: run_artifacts.clone(),
            });
            artifact_groups.push(build_artifact_group(task.id, run, &run_artifacts));
        }

        dedupe_lineage_nodes(&mut lineage_nodes);

        Ok(TaskExecutionDetail {
            task,
            plan_detail,
            runs,
            timeline_groups,
            artifact_groups,
            lineage_nodes,
            lineage_edges,
        })
    }
}

fn artifacts_for_run(artifacts: &[ArtifactRecord], run_id: Uuid) -> Vec<ArtifactRecord> {
    let mut run_artifacts = artifacts
        .iter()
        .filter(|artifact| artifact.run_id == run_id)
        .cloned()
        .collect::<Vec<_>>();
    run_artifacts.sort_by_key(|artifact| std::cmp::Reverse(artifact.updated_at));
    run_artifacts
}

fn build_artifact_group(
    task_id: Uuid,
    run: &RunRecord,
    artifacts: &[ArtifactRecord],
) -> ArtifactGroupRecord {
    ArtifactGroupRecord {
        id: format!("run-group-{}", run.id),
        task_id,
        run_id: run.id,
        title: format!("Run {} outputs", short_id(run.id)),
        summary: format!(
            "{} artifact(s) from {} / {}",
            artifacts.len(),
            run.selected_provider.as_deref().unwrap_or("local runtime"),
            run.selected_model.as_deref().unwrap_or("fallback"),
        ),
        primary_artifact_id: artifacts
            .iter()
            .find(|artifact| matches!(artifact.kind, ArtifactKind::Report))
            .or_else(|| {
                artifacts
                    .iter()
                    .find(|artifact| matches!(artifact.kind, ArtifactKind::Response))
            })
            .or_else(|| artifacts.first())
            .map(|artifact| artifact.id),
        artifact_ids: artifacts.iter().map(|artifact| artifact.id).collect(),
        created_at: run.finished_at.unwrap_or(run.started_at),
    }
}

fn producer_node_for_artifact(
    run: &RunRecord,
    run_steps: &[RunStepRecord],
    artifact: &ArtifactRecord,
) -> String {
    if let (Some(kind), Some(ref_id)) = (&artifact.producer_kind, artifact.producer_ref_id) {
        return match kind {
            ArtifactProducerKind::Run => run_node_id(ref_id),
            ArtifactProducerKind::RunStep => run_step_node_id(ref_id),
            ArtifactProducerKind::ToolInvocation => tool_node_id(ref_id),
        };
    }

    if matches!(artifact.kind, ArtifactKind::Report | ArtifactKind::Response) {
        if let Some(step) = run_steps
            .iter()
            .rev()
            .find(|step| matches!(step.kind, PlanStepKind::Respond))
        {
            return run_step_node_id(step.id);
        }
    }

    run_node_id(run.id)
}

fn dedupe_lineage_nodes(nodes: &mut Vec<LineageNodeRecord>) {
    let mut seen = std::collections::HashSet::new();
    nodes.retain(|node| seen.insert(node.id.clone()));
}

fn short_id(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

fn task_node_id(task_id: Uuid) -> String {
    format!("task:{task_id}")
}

fn run_node_id(run_id: Uuid) -> String {
    format!("run:{run_id}")
}

fn run_step_node_id(run_step_id: Uuid) -> String {
    format!("run-step:{run_step_id}")
}

fn tool_node_id(tool_invocation_id: Uuid) -> String {
    format!("tool:{tool_invocation_id}")
}

fn artifact_node_id(artifact_id: Uuid) -> String {
    format!("artifact:{artifact_id}")
}

fn enum_label<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|json| json.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}
