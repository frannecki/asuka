use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use chrono::{DateTime, Utc};
use pulldown_cmark::{html, Options, Parser};
use serde_json::json;
use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{
        ArtifactKind, ArtifactProducerKind, ArtifactRecord, ArtifactRenderMode, PlanDetail,
        PlanStepKind, RunRecord, RunStepRecord, ToolInvocationRecord, WorkspaceEntryKind,
        WorkspaceNode,
    },
    error::{CoreError, CoreResult},
    tools::{ToolArtifact, ToolArtifactContent},
};

impl AgentCore {
    pub fn session_workspace_root(&self, session_id: Uuid) -> PathBuf {
        self.workspace_root
            .join(".asuka")
            .join("workspaces")
            .join(session_id.to_string())
    }

    pub async fn get_session_workspace_tree(&self, session_id: Uuid) -> CoreResult<WorkspaceNode> {
        let session = self.store.get_session(session_id).await?;
        let root = self.session_workspace_root(session_id);
        fs::create_dir_all(&root).map_err(|error| {
            CoreError::new(
                500,
                format!(
                    "failed to create session workspace {}: {error}",
                    root.display()
                ),
            )
        })?;

        Ok(build_workspace_node(&root, &root, &session.session.title)?)
    }

    pub async fn read_session_workspace_file(
        &self,
        session_id: Uuid,
        relative_path: &str,
    ) -> CoreResult<Vec<u8>> {
        self.store.get_session(session_id).await?;
        let root = self.session_workspace_root(session_id);
        fs::create_dir_all(&root).map_err(|error| {
            CoreError::new(
                500,
                format!(
                    "failed to create session workspace {}: {error}",
                    root.display()
                ),
            )
        })?;
        let target = resolve_workspace_path(&root, relative_path)?;
        if target.is_dir() {
            return Err(CoreError::bad_request("workspace path is a directory"));
        }

        fs::read(&target).map_err(|error| {
            CoreError::new(
                404,
                format!(
                    "workspace file {} could not be read: {error}",
                    target.display()
                ),
            )
        })
    }

    pub async fn render_session_workspace_markdown(
        &self,
        session_id: Uuid,
        relative_path: &str,
    ) -> CoreResult<String> {
        let raw = self
            .read_session_workspace_file(session_id, relative_path)
            .await?;
        let markdown = String::from_utf8(raw).map_err(|error| {
            CoreError::bad_request(format!(
                "workspace file is not valid UTF-8 markdown: {error}"
            ))
        })?;
        Ok(render_markdown_document(relative_path, &markdown))
    }

    pub(crate) async fn write_tool_invocation_artifacts(
        &self,
        session_id: Uuid,
        task_id: Uuid,
        run_id: Uuid,
        invocation: &ToolInvocationRecord,
        extra_artifacts: &[ToolArtifact],
    ) -> CoreResult<()> {
        let invocation_root = format!("runs/{run_id}/tool-invocations/{}", invocation.id);
        let invocation_payload = serde_json::to_string_pretty(&json!({ "invocation": invocation }))
            .unwrap_or_else(|_| "{}".to_string());

        self.write_session_artifact_file(
            session_id,
            task_id,
            run_id,
            &format!("{invocation_root}/result.json"),
            &invocation_payload,
            ArtifactSpec {
                display_name: format!("{} result", invocation.tool_name),
                description: format!(
                    "Structured result payload for tool invocation {}.",
                    invocation.tool_name
                ),
                kind: ArtifactKind::Data,
                render_mode: ArtifactRenderMode::Json,
                media_type: "application/json; charset=utf-8".to_string(),
                producer_kind: Some(ArtifactProducerKind::ToolInvocation),
                producer_ref_id: Some(invocation.id),
            },
        )
        .await?;

        for artifact in extra_artifacts {
            let output_path = normalize_artifact_path(&artifact.relative_path)?;
            let relative_path = format!("{invocation_root}/outputs/{output_path}");
            self.write_tool_artifact_file(
                session_id,
                task_id,
                run_id,
                &relative_path,
                artifact,
                invocation.id,
            )
            .await?;
        }

        Ok(())
    }

    pub(crate) async fn write_subagent_artifact(
        &self,
        session_id: Uuid,
        task_id: Uuid,
        run_id: Uuid,
        run_step_id: Uuid,
        subagent_name: &str,
        summary: &str,
    ) -> CoreResult<()> {
        let relative_path = format!("runs/{run_id}/subagents/{run_step_id}/summary.md");
        let content = format!("# {subagent_name}\n\n{summary}\n");
        self.write_session_artifact_file(
            session_id,
            task_id,
            run_id,
            &relative_path,
            &content,
            ArtifactSpec {
                display_name: format!("{subagent_name} summary"),
                description: format!(
                    "Prototype subagent output produced by {subagent_name} during the run."
                ),
                kind: ArtifactKind::Data,
                render_mode: ArtifactRenderMode::Markdown,
                media_type: "text/markdown; charset=utf-8".to_string(),
                producer_kind: Some(ArtifactProducerKind::RunStep),
                producer_ref_id: Some(run_step_id),
            },
        )
        .await
    }

    pub(crate) async fn write_run_artifacts(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        user_content: &str,
        response: &str,
    ) -> CoreResult<()> {
        let run = self.get_run(run_id).await?;
        let task = self.get_task(run.task_id).await?;
        let plan = self.get_task_plan(run.task_id).await?;
        let run_steps = self.list_run_steps(run_id).await?;
        let tool_invocations = self.list_tool_invocations(run_id).await?;

        let run_dir = self
            .session_workspace_root(session_id)
            .join("runs")
            .join(run_id.to_string());
        fs::create_dir_all(&run_dir).map_err(|error| {
            CoreError::new(
                500,
                format!(
                    "failed to create run artifact directory {}: {error}",
                    run_dir.display()
                ),
            )
        })?;

        let assistant_markdown = build_assistant_markdown(
            &run,
            &task.title,
            &plan,
            &run_steps,
            &tool_invocations,
            user_content,
            response,
        );
        let report_html = build_report_html(
            &run,
            &task.title,
            &plan,
            &run_steps,
            &tool_invocations,
            user_content,
            response,
        );
        let respond_step_id = run_steps
            .iter()
            .rev()
            .find(|step| matches!(step.kind, PlanStepKind::Respond))
            .map(|step| step.id);
        let last_tool_invocation_id = tool_invocations.last().map(|invocation| invocation.id);

        self.write_run_artifact_file(
            session_id,
            task.id,
            run.id,
            "runs",
            "assistant-response.md",
            &assistant_markdown,
            ArtifactSpec {
                display_name: "Assistant response".to_string(),
                description: "Markdown summary of the completed run and response.".to_string(),
                kind: ArtifactKind::Response,
                render_mode: ArtifactRenderMode::Markdown,
                media_type: "text/markdown; charset=utf-8".to_string(),
                producer_kind: respond_step_id.map(|_| ArtifactProducerKind::RunStep),
                producer_ref_id: respond_step_id,
            },
        )
        .await?;
        self.write_run_artifact_file(
            session_id,
            task.id,
            run.id,
            "runs",
            "plan.json",
            &serde_json::to_string_pretty(&plan).unwrap_or_else(|_| "{}".to_string()),
            ArtifactSpec {
                display_name: "Plan detail".to_string(),
                description: "Serialized task plan detail for the run.".to_string(),
                kind: ArtifactKind::Data,
                render_mode: ArtifactRenderMode::Json,
                media_type: "application/json; charset=utf-8".to_string(),
                producer_kind: Some(ArtifactProducerKind::Run),
                producer_ref_id: Some(run.id),
            },
        )
        .await?;
        self.write_run_artifact_file(
            session_id,
            task.id,
            run.id,
            "runs",
            "run.json",
            &serde_json::to_string_pretty(&run).unwrap_or_else(|_| "{}".to_string()),
            ArtifactSpec {
                display_name: "Run record".to_string(),
                description: "Serialized run metadata captured at completion time.".to_string(),
                kind: ArtifactKind::Data,
                render_mode: ArtifactRenderMode::Json,
                media_type: "application/json; charset=utf-8".to_string(),
                producer_kind: Some(ArtifactProducerKind::Run),
                producer_ref_id: Some(run.id),
            },
        )
        .await?;
        self.write_run_artifact_file(
            session_id,
            task.id,
            run.id,
            "runs",
            "run-steps.json",
            &serde_json::to_string_pretty(&run_steps).unwrap_or_else(|_| "[]".to_string()),
            ArtifactSpec {
                display_name: "Run steps".to_string(),
                description: "Recorded execution steps for the run.".to_string(),
                kind: ArtifactKind::Data,
                render_mode: ArtifactRenderMode::Json,
                media_type: "application/json; charset=utf-8".to_string(),
                producer_kind: respond_step_id
                    .map(|_| ArtifactProducerKind::RunStep)
                    .or(Some(ArtifactProducerKind::Run)),
                producer_ref_id: respond_step_id.or(Some(run.id)),
            },
        )
        .await?;
        self.write_run_artifact_file(
            session_id,
            task.id,
            run.id,
            "runs",
            "tool-invocations.json",
            &serde_json::to_string_pretty(&tool_invocations).unwrap_or_else(|_| "[]".to_string()),
            ArtifactSpec {
                display_name: "Tool invocations".to_string(),
                description: "Tool call arguments and results captured during the run.".to_string(),
                kind: ArtifactKind::Data,
                render_mode: ArtifactRenderMode::Json,
                media_type: "application/json; charset=utf-8".to_string(),
                producer_kind: last_tool_invocation_id
                    .map(|_| ArtifactProducerKind::ToolInvocation)
                    .or(Some(ArtifactProducerKind::Run)),
                producer_ref_id: last_tool_invocation_id.or(Some(run.id)),
            },
        )
        .await?;
        self.write_run_artifact_file(
            session_id,
            task.id,
            run.id,
            "runs",
            "report.html",
            &report_html,
            ArtifactSpec {
                display_name: "Run report".to_string(),
                description: "Standalone HTML report for the completed run.".to_string(),
                kind: ArtifactKind::Report,
                render_mode: ArtifactRenderMode::Html,
                media_type: "text/html; charset=utf-8".to_string(),
                producer_kind: respond_step_id.map(|_| ArtifactProducerKind::RunStep),
                producer_ref_id: respond_step_id,
            },
        )
        .await?;

        Ok(())
    }
}

struct ArtifactSpec {
    display_name: String,
    description: String,
    kind: ArtifactKind,
    render_mode: ArtifactRenderMode,
    media_type: String,
    producer_kind: Option<ArtifactProducerKind>,
    producer_ref_id: Option<Uuid>,
}

impl AgentCore {
    async fn write_run_artifact_file(
        &self,
        session_id: Uuid,
        task_id: Uuid,
        run_id: Uuid,
        run_dir_prefix: &str,
        file_name: &str,
        contents: &str,
        spec: ArtifactSpec,
    ) -> CoreResult<()> {
        let relative_path = format!("{run_dir_prefix}/{run_id}/{file_name}");
        self.write_session_artifact_file(
            session_id,
            task_id,
            run_id,
            &relative_path,
            contents,
            spec,
        )
        .await
    }

    async fn write_tool_artifact_file(
        &self,
        session_id: Uuid,
        task_id: Uuid,
        run_id: Uuid,
        relative_path: &str,
        artifact: &ToolArtifact,
        tool_invocation_id: Uuid,
    ) -> CoreResult<()> {
        let target_path = self.session_workspace_root(session_id).join(relative_path);
        let size_bytes = match &artifact.content {
            ToolArtifactContent::Utf8(contents) => {
                write_utf8_file(target_path, contents)?;
                artifact.size_bytes()
            }
        };

        let now = Utc::now();
        self.store
            .upsert_artifact(ArtifactRecord {
                id: artifact_id(run_id, relative_path),
                session_id,
                task_id,
                run_id,
                path: relative_path.to_string(),
                display_name: artifact.display_name.clone(),
                description: artifact.description.clone(),
                kind: artifact.kind.clone(),
                media_type: artifact.media_type.clone(),
                render_mode: artifact.render_mode.clone(),
                size_bytes,
                producer_kind: Some(ArtifactProducerKind::ToolInvocation),
                producer_ref_id: Some(tool_invocation_id),
                created_at: now,
                updated_at: now,
            })
            .await?;

        Ok(())
    }

    async fn write_session_artifact_file(
        &self,
        session_id: Uuid,
        task_id: Uuid,
        run_id: Uuid,
        relative_path: &str,
        contents: &str,
        spec: ArtifactSpec,
    ) -> CoreResult<()> {
        let target_path = self.session_workspace_root(session_id).join(relative_path);
        write_utf8_file(target_path, contents)?;

        let now = Utc::now();
        self.store
            .upsert_artifact(ArtifactRecord {
                id: artifact_id(run_id, relative_path),
                session_id,
                task_id,
                run_id,
                path: relative_path.to_string(),
                display_name: spec.display_name,
                description: spec.description,
                kind: spec.kind,
                media_type: spec.media_type,
                render_mode: spec.render_mode,
                size_bytes: contents.len() as u64,
                producer_kind: spec.producer_kind,
                producer_ref_id: spec.producer_ref_id,
                created_at: now,
                updated_at: now,
            })
            .await?;

        Ok(())
    }
}

fn artifact_id(run_id: Uuid, relative_path: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("run:{run_id}:artifact:{relative_path}").as_bytes(),
    )
}

fn build_workspace_node(root: &Path, target: &Path, label: &str) -> CoreResult<WorkspaceNode> {
    let metadata = fs::metadata(target).map_err(|error| {
        CoreError::new(
            500,
            format!(
                "failed to read workspace metadata for {}: {error}",
                target.display()
            ),
        )
    })?;
    let kind = if metadata.is_dir() {
        WorkspaceEntryKind::Directory
    } else {
        WorkspaceEntryKind::File
    };
    let modified = metadata.modified().ok().map(DateTime::<Utc>::from);
    let path = target
        .strip_prefix(root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    let mut children = Vec::new();
    if metadata.is_dir() {
        let mut entries = fs::read_dir(target)
            .map_err(|error| {
                CoreError::new(
                    500,
                    format!(
                        "failed to read workspace directory {}: {error}",
                        target.display()
                    ),
                )
            })?
            .flatten()
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
        for entry in entries {
            let child_path = entry.path();
            children.push(build_workspace_node(
                root,
                &child_path,
                &entry.file_name().to_string_lossy(),
            )?);
        }
        children.sort_by(|left, right| match (&left.kind, &right.kind) {
            (WorkspaceEntryKind::Directory, WorkspaceEntryKind::File) => std::cmp::Ordering::Less,
            (WorkspaceEntryKind::File, WorkspaceEntryKind::Directory) => {
                std::cmp::Ordering::Greater
            }
            _ => left.name.cmp(&right.name),
        });
    }

    Ok(WorkspaceNode {
        name: if path.is_empty() {
            label.to_string()
        } else {
            target
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(label)
                .to_string()
        },
        path,
        kind,
        size: metadata.is_file().then_some(metadata.len()),
        updated_at: modified,
        children,
    })
}

fn resolve_workspace_path(root: &Path, relative_path: &str) -> CoreResult<PathBuf> {
    let mut resolved = root.to_path_buf();

    for component in Path::new(relative_path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => resolved.push(part),
            Component::ParentDir => {
                if !resolved.pop() || !resolved.starts_with(root) {
                    return Err(CoreError::bad_request(
                        "workspace path escapes the session workspace",
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(CoreError::bad_request(
                    "workspace path must be relative to the session workspace",
                ))
            }
        }
    }

    if !resolved.starts_with(root) {
        return Err(CoreError::bad_request(
            "workspace path escapes the session workspace",
        ));
    }

    Ok(resolved)
}

fn render_markdown_document(path: &str, markdown: &str) -> String {
    let mut rendered = String::new();
    let parser = Parser::new_ext(markdown, Options::all());
    html::push_html(&mut rendered, parser);

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><style>{}</style></head><body><main class=\"markdown-body\">{}</main></body></html>",
        escape_html(path),
        markdown_styles(),
        rendered
    )
}

fn build_assistant_markdown(
    run: &RunRecord,
    task_title: &str,
    plan: &PlanDetail,
    run_steps: &[RunStepRecord],
    tool_invocations: &[ToolInvocationRecord],
    user_content: &str,
    response: &str,
) -> String {
    let plan_lines = plan
        .steps
        .iter()
        .map(|step| {
            format!(
                "- {} [{}]",
                step.title,
                serde_json::to_value(&step.status)
                    .ok()
                    .and_then(|value| value.as_str().map(ToOwned::to_owned))
                    .unwrap_or_else(|| "unknown".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let run_step_lines = run_steps
        .iter()
        .map(|step| {
            format!(
                "- #{} {} [{}]",
                step.sequence,
                step.title,
                serde_json::to_value(&step.status)
                    .ok()
                    .and_then(|value| value.as_str().map(ToOwned::to_owned))
                    .unwrap_or_else(|| "unknown".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let tool_lines = if tool_invocations.is_empty() {
        "- none".to_string()
    } else {
        tool_invocations
            .iter()
            .map(|invocation| {
                format!(
                    "- {} [{}]",
                    invocation.tool_name,
                    if invocation.ok { "ok" } else { "error" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "# Run {}\n\n## Task\n- Title: {}\n- Status: {}\n- Provider: {}\n- Model: {}\n\n## User request\n{}\n\n## Assistant response\n{}\n\n## Plan\n{}\n\n## Run steps\n{}\n\n## Tool invocations\n{}\n",
        run.id,
        task_title,
        serde_json::to_value(&run.status)
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "unknown".to_string()),
        run.selected_provider.as_deref().unwrap_or("none"),
        run.selected_model.as_deref().unwrap_or("none"),
        user_content,
        response,
        plan_lines,
        run_step_lines,
        tool_lines
    )
}

fn build_report_html(
    run: &RunRecord,
    task_title: &str,
    plan: &PlanDetail,
    run_steps: &[RunStepRecord],
    tool_invocations: &[ToolInvocationRecord],
    user_content: &str,
    response: &str,
) -> String {
    let plan_markup = plan
        .steps
        .iter()
        .map(|step| {
            format!(
                "<li><strong>{}</strong><span>{}</span></li>",
                escape_html(&step.title),
                escape_html(
                    &serde_json::to_value(&step.status)
                        .ok()
                        .and_then(|value| value.as_str().map(ToOwned::to_owned))
                        .unwrap_or_else(|| "unknown".to_string())
                )
            )
        })
        .collect::<String>();
    let step_markup = run_steps
        .iter()
        .map(|step| {
            format!(
                "<article class=\"step-card\"><header><strong>#{}</strong><span>{}</span></header><h3>{}</h3><p>{}</p>{}</article>",
                step.sequence,
                escape_html(&serde_json::to_value(&step.status).ok().and_then(|value| value.as_str().map(ToOwned::to_owned)).unwrap_or_else(|| "unknown".to_string())),
                escape_html(&step.title),
                escape_html(&step.input_summary),
                step.output_summary.as_ref().map(|summary| format!("<pre>{}</pre>", escape_html(summary))).unwrap_or_default()
            )
        })
        .collect::<String>();
    let tool_markup = if tool_invocations.is_empty() {
        "<p class=\"muted\">No tool invocations were recorded for this run.</p>".to_string()
    } else {
        tool_invocations
            .iter()
            .map(|invocation| {
                format!(
                    "<article class=\"tool-card\"><header><strong>{}</strong><span>{}</span></header><pre>{}</pre><pre>{}</pre></article>",
                    escape_html(&invocation.tool_name),
                    if invocation.ok { "ok" } else { "error" },
                    escape_html(&serde_json::to_string_pretty(&invocation.arguments_json).unwrap_or_else(|_| "{}".to_string())),
                    escape_html(&serde_json::to_string_pretty(&invocation.result_json).unwrap_or_else(|_| "{}".to_string()))
                )
            })
            .collect::<String>()
    };

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Run {}</title><style>{}</style></head><body><main class=\"report-shell\"><section class=\"hero\"><p class=\"eyebrow\">Session Workspace Artifact</p><h1>{}</h1><p class=\"muted\">Run {} • {} • {}</p></section><section class=\"panel\"><h2>User request</h2><p>{}</p></section><section class=\"panel\"><h2>Assistant response</h2><p>{}</p></section><section class=\"panel\"><h2>Plan</h2><ol class=\"plan-list\">{}</ol></section><section class=\"panel\"><h2>Run steps</h2><div class=\"step-grid\">{}</div></section><section class=\"panel\"><h2>Tool invocations</h2><div class=\"tool-grid\">{}</div></section></main></body></html>",
        run.id,
        report_styles(),
        escape_html(task_title),
        run.id,
        escape_html(run.selected_provider.as_deref().unwrap_or("no provider")),
        escape_html(run.selected_model.as_deref().unwrap_or("no model")),
        escape_html(user_content),
        escape_html(response),
        plan_markup,
        step_markup,
        tool_markup
    )
}

fn write_utf8_file(path: PathBuf, contents: &str) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CoreError::new(
                500,
                format!(
                    "failed to prepare artifact directory {}: {error}",
                    parent.display()
                ),
            )
        })?;
    }
    fs::write(&path, contents).map_err(|error| {
        CoreError::new(
            500,
            format!("failed to write artifact {}: {error}", path.display()),
        )
    })
}

fn normalize_artifact_path(relative_path: &str) -> CoreResult<String> {
    let mut normalized = PathBuf::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(CoreError::bad_request(
                    "artifact path must remain inside the invocation workspace",
                ))
            }
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() {
        return Err(CoreError::bad_request("artifact path cannot be empty"));
    }

    Ok(normalized)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn markdown_styles() -> &'static str {
    r#"
body { margin: 0; background: #f7f1e7; color: #1a2325; font-family: ui-serif, Georgia, serif; }
.markdown-body { max-width: 840px; margin: 0 auto; padding: 40px 24px 64px; line-height: 1.7; }
h1, h2, h3 { line-height: 1.15; }
pre, code { font-family: ui-monospace, SFMono-Regular, monospace; }
pre { background: rgba(17, 24, 39, 0.08); padding: 16px; border-radius: 16px; overflow: auto; }
blockquote { border-left: 4px solid #dd5d3b; padding-left: 16px; color: #526066; }
"#
}

fn report_styles() -> &'static str {
    r#"
body { margin: 0; background: linear-gradient(135deg, #f5eddf, #f1e6d1 42%, #eadfc9); color: #162022; font-family: ui-sans-serif, system-ui, sans-serif; }
.report-shell { display: grid; gap: 18px; padding: 28px; max-width: 1200px; margin: 0 auto; }
.hero, .panel { border-radius: 24px; background: rgba(255,255,255,0.78); border: 1px solid rgba(22,32,34,0.08); padding: 24px; box-shadow: 0 18px 50px rgba(35,32,24,0.1); }
.eyebrow { color: #b84222; text-transform: uppercase; letter-spacing: 0.16em; font-size: 0.78rem; }
.muted { color: #526066; }
.plan-list, .step-grid, .tool-grid { display: grid; gap: 14px; }
.step-grid { grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); }
.step-card, .tool-card { border-radius: 18px; background: rgba(247,241,231,0.9); padding: 18px; border: 1px solid rgba(22,32,34,0.08); display: grid; gap: 10px; }
.step-card header, .tool-card header { display: flex; justify-content: space-between; gap: 12px; color: #526066; text-transform: uppercase; font-size: 0.78rem; }
pre { margin: 0; padding: 14px; border-radius: 14px; background: rgba(17,24,39,0.08); overflow: auto; white-space: pre-wrap; word-break: break-word; }
p { line-height: 1.65; }
"#
}
