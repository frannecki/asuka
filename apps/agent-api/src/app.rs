use axum::{http::Method, Router};
use tower_http::cors::{Any, CorsLayer};

use crate::{http, state::ApiState};

pub fn build_app(state: ApiState) -> Router {
    Router::new()
        .merge(http::root::router())
        .nest("/api/v1", http::api_router())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                ])
                .allow_headers(Any),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Mutex, OnceLock},
        time::Duration,
    };

    use agent_core::{
        test_support::{
            create_test_core_with_openrouter_transport, TestOpenRouterOutcome,
            TestOpenRouterResponse, TestOpenRouterTransport,
        },
        ActiveRunEnvelope, AgentCore, ArtifactProducerKind, ArtifactRecord, PlanDetail,
        PlanStepKind, PlanStepStatus, RunAccepted, RunEventHistory, RunRecord, RunStatus,
        RunStepRecord, RunStepStatus, SessionDetail, SessionRecord, SessionSkillAvailability,
        SessionSkillPolicyMode, SessionSkillsDetail, SkillPreset, TaskExecutionDetail, TaskRecord,
        TaskStatus, ToolInvocationRecord, WorkspaceNode,
    };
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
    };
    use hyper::body::{to_bytes, HttpBody};
    use serde::de::DeserializeOwned;
    use serde_json::{json, Value};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::build_app;
    use crate::state::ApiState;

    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_api_test() -> std::sync::MutexGuard<'static, ()> {
        test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(previous) => std::env::set_var(self.key, previous),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn test_models_config_toml() -> &'static str {
        r#"
[[providers]]
provider_type = "openRouter"
display_name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
default_model = "demo-model"

[[providers.models]]
name = "demo-model"
context_window = 8192
supports_tools = false
supports_embeddings = false
capabilities = ["chat"]
"#
    }

    fn write_test_models_config() -> PathBuf {
        let path = std::env::temp_dir().join(format!("agent-api-models-{}.toml", Uuid::new_v4()));
        fs::write(&path, test_models_config_toml()).expect("write agent-api test models config");
        path
    }

    async fn build_test_router() -> axum::Router {
        let core = AgentCore::new(write_test_models_config())
            .await
            .expect("build test core");
        build_app(ApiState::new(core))
    }

    fn build_test_router_with_openrouter_transport(
        transport: std::sync::Arc<TestOpenRouterTransport>,
    ) -> axum::Router {
        let core = create_test_core_with_openrouter_transport(test_models_config_toml(), transport);
        build_app(ApiState::new(core))
    }

    async fn create_session(app: &axum::Router, title: &str) -> SessionRecord {
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({ "title": title }))
                            .expect("serialize session payload"),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("create session request");

        assert_eq!(create_response.status(), StatusCode::OK);
        decode_json(create_response).await
    }

    async fn decode_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
        let body = to_bytes(response.into_body())
            .await
            .expect("read response body");
        serde_json::from_slice(&body).expect("decode json body")
    }

    async fn post_session_message(
        app: &axum::Router,
        session_id: Uuid,
        content: &str,
    ) -> RunAccepted {
        let post_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{}/messages", session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({ "content": content }))
                            .expect("serialize message payload"),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("post message request");

        assert_eq!(post_response.status(), StatusCode::OK);
        decode_json(post_response).await
    }

    async fn wait_for_completed_run(app: &axum::Router, run_id: Uuid) -> RunRecord {
        for _ in 0..40 {
            let run_response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/v1/runs/{}", run_id))
                        .body(Body::empty())
                        .expect("build request"),
                )
                .await
                .expect("get run request");

            let run: RunRecord = decode_json(run_response).await;
            if matches!(run.status, RunStatus::Completed) {
                return run;
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        panic!("run {run_id} did not complete in time");
    }

    async fn get_session_detail(app: &axum::Router, session_id: Uuid) -> SessionDetail {
        let session_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}", session_id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get session request");

        decode_json(session_response).await
    }

    async fn get_active_run_detail(app: &axum::Router, session_id: Uuid) -> ActiveRunEnvelope {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{session_id}/active-run"))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get active run request");

        decode_json(response).await
    }

    async fn get_run_event_history(
        app: &axum::Router,
        run_id: Uuid,
        after_sequence: Option<u64>,
    ) -> RunEventHistory {
        let uri = after_sequence
            .map(|value| format!("/api/v1/runs/{run_id}/events/history?afterSequence={value}"))
            .unwrap_or_else(|| format!("/api/v1/runs/{run_id}/events/history"));
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get run event history request");

        decode_json(response).await
    }

    #[derive(Debug)]
    struct TestSseEvent {
        event_type: String,
        data: Value,
    }

    fn parse_sse_frame(frame: &str) -> Option<TestSseEvent> {
        let mut event_type = None;
        let mut data_lines = Vec::new();

        for line in frame.lines() {
            let Some((field, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim_start();

            match field {
                "event" => event_type = Some(value.to_string()),
                "data" => data_lines.push(value),
                _ => {}
            }
        }

        let event_type = event_type?;
        let data = if data_lines.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&data_lines.join("\n")).expect("decode sse event payload")
        };

        Some(TestSseEvent { event_type, data })
    }

    async fn decode_sse_until(
        response: axum::response::Response,
        terminal_event: &str,
    ) -> Vec<TestSseEvent> {
        let mut body = response.into_body();
        let mut buffer = String::new();
        let mut events = Vec::new();

        loop {
            let next_chunk = tokio::time::timeout(Duration::from_secs(5), body.data())
                .await
                .expect("read sse chunk timeout");

            let Some(chunk) = next_chunk else {
                break;
            };

            let chunk = chunk.expect("read sse chunk");
            let normalized = String::from_utf8_lossy(chunk.as_ref()).replace('\r', "");
            buffer.push_str(&normalized);

            while let Some(boundary) = buffer.find("\n\n") {
                let frame = buffer[..boundary].to_string();
                buffer.drain(..boundary + 2);

                if let Some(event) = parse_sse_frame(&frame) {
                    let is_terminal = event.event_type == terminal_event;
                    events.push(event);
                    if is_terminal {
                        return events;
                    }
                }
            }
        }

        panic!("stream ended before receiving terminal event {terminal_event}");
    }

    fn workspace_contains_path(node: &WorkspaceNode, path: &str) -> bool {
        if node.path == path {
            return true;
        }

        node.children
            .iter()
            .any(|child| workspace_contains_path(child, path))
    }

    #[tokio::test(flavor = "current_thread")]
    async fn healthz_route_returns_ok_status() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("health request");

        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = decode_json(response).await;
        assert_eq!(body, json!({ "status": "ok" }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sessions_route_creates_and_lists_sessions() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;

        let created = create_session(&app, "API Test Session").await;
        assert_eq!(created.title, "API Test Session");

        let list_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/sessions")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("list sessions request");

        assert_eq!(list_response.status(), StatusCode::OK);
        let sessions: Vec<SessionRecord> = decode_json(list_response).await;
        assert!(sessions.iter().any(|session| session.id == created.id));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn post_message_completes_run_and_persists_assistant_reply() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;

        let session = create_session(&app, "Run Session").await;

        let post_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{}/messages", session.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(
                            &json!({ "content": "remember this architecture note" }),
                        )
                        .expect("serialize message payload"),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("post message request");

        assert_eq!(post_response.status(), StatusCode::OK);
        let accepted: RunAccepted = decode_json(post_response).await;
        assert_eq!(accepted.user_message.session_id, session.id);

        let mut final_run: Option<RunRecord> = None;
        for _ in 0..40 {
            let run_response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/v1/runs/{}", accepted.run.id))
                        .body(Body::empty())
                        .expect("build request"),
                )
                .await
                .expect("get run request");

            let run: RunRecord = decode_json(run_response).await;
            if matches!(run.status, RunStatus::Completed) {
                final_run = Some(run);
                break;
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let final_run = final_run.expect("run to complete");
        assert!(matches!(final_run.status, RunStatus::Completed));

        let session_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get session request");

        let detail: SessionDetail = decode_json(session_response).await;
        assert!(detail.messages.len() >= 2);
        assert!(detail
            .messages
            .iter()
            .any(|message| matches!(message.role, agent_core::MessageRole::Assistant)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn post_message_persists_task_plan_and_run_steps_via_api() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;

        let session = create_session(&app, "Harness Session").await;
        let accepted = post_session_message(
            &app,
            session.id,
            "Explain how the structured harness tracks this request.",
        )
        .await;
        let final_run = wait_for_completed_run(&app, accepted.run.id).await;

        assert!(matches!(final_run.status, RunStatus::Completed));
        assert_eq!(final_run.task_id, accepted.run.task_id);

        let tasks_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/tasks?sessionId={}", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("list tasks request");
        assert_eq!(tasks_response.status(), StatusCode::OK);
        let tasks: Vec<TaskRecord> = decode_json(tasks_response).await;
        let task = tasks
            .iter()
            .find(|task| task.id == accepted.run.task_id)
            .expect("accepted task in task list");
        assert_eq!(task.session_id, session.id);
        assert_eq!(task.latest_run_id, Some(accepted.run.id));
        assert!(matches!(task.status, TaskStatus::Completed));

        let task_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/tasks/{}", accepted.run.task_id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get task request");
        assert_eq!(task_response.status(), StatusCode::OK);
        let task_detail: TaskRecord = decode_json(task_response).await;
        assert_eq!(task_detail.id, accepted.run.task_id);
        assert_eq!(task_detail.current_plan_id, task.current_plan_id);

        let plan_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/tasks/{}/plan", accepted.run.task_id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get task plan request");
        assert_eq!(plan_response.status(), StatusCode::OK);
        let plan_detail: PlanDetail = decode_json(plan_response).await;
        assert_eq!(plan_detail.plan.task_id, accepted.run.task_id);
        assert_eq!(plan_detail.steps.len(), 2);
        assert_eq!(plan_detail.steps[0].kind, PlanStepKind::ContextBuild);
        assert_eq!(plan_detail.steps[1].kind, PlanStepKind::Respond);
        assert!(plan_detail
            .steps
            .iter()
            .all(|step| { matches!(step.status, PlanStepStatus::Completed) }));

        let run_steps_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{}/steps", accepted.run.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("list run steps request");
        assert_eq!(run_steps_response.status(), StatusCode::OK);
        let run_steps: Vec<RunStepRecord> = decode_json(run_steps_response).await;
        assert!(run_steps.len() >= 2);
        assert!(run_steps
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence));
        assert!(run_steps.iter().any(|step| {
            step.plan_step_id == Some(plan_detail.steps[0].id)
                && step.kind == PlanStepKind::ContextBuild
                && matches!(step.status, RunStepStatus::Completed)
        }));
        assert!(run_steps.iter().any(|step| {
            step.plan_step_id == Some(plan_detail.steps[1].id)
                && step.kind == PlanStepKind::Respond
                && matches!(step.status, RunStepStatus::Completed)
        }));

        let tool_invocations_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{}/tool-invocations", accepted.run.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("list tool invocations request");
        assert_eq!(tool_invocations_response.status(), StatusCode::OK);
        let tool_invocations: Vec<ToolInvocationRecord> =
            decode_json(tool_invocations_response).await;
        assert!(tool_invocations.is_empty());

        let execution_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/tasks/{}/execution", accepted.run.task_id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("task execution request");
        assert_eq!(execution_response.status(), StatusCode::OK);
        let execution: TaskExecutionDetail = decode_json(execution_response).await;
        assert_eq!(execution.task.id, accepted.run.task_id);
        assert_eq!(execution.runs.len(), 1);
        assert_eq!(execution.timeline_groups.len(), 1);
        assert_eq!(execution.timeline_groups[0].run.id, accepted.run.id);
        let assistant_artifact = execution.timeline_groups[0]
            .artifacts
            .iter()
            .find(|artifact| artifact.path.ends_with("assistant-response.md"))
            .expect("assistant artifact in task execution");
        assert_eq!(
            assistant_artifact.producer_kind,
            Some(ArtifactProducerKind::RunStep)
        );
        assert!(execution
            .lineage_edges
            .iter()
            .any(|edge| edge.relation == "executes"));
        assert!(execution.lineage_edges.iter().any(|edge| {
            edge.relation == "produces"
                && edge.to == format!("artifact:{}", assistant_artifact.id)
                && edge.from.starts_with("run-step:")
        }));
        assert!(execution
            .artifact_groups
            .iter()
            .any(|group| group.run_id == accepted.run.id));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn completed_run_exposes_session_workspace_artifacts_via_api() {
        let _lock = lock_api_test();
        let temp_root =
            std::env::temp_dir().join(format!("asuka-workspace-api-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_root).expect("create temporary workspace root");
        let _workspace_root = EnvVarGuard::set(
            "ASUKA_WORKSPACE_ROOT",
            temp_root.to_str().expect("temporary workspace root"),
        );
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;

        let session = create_session(&app, "Workspace Session").await;
        let accepted = post_session_message(
            &app,
            session.id,
            "Explain how the workspace artifacts are produced for this run.",
        )
        .await;
        let final_run = wait_for_completed_run(&app, accepted.run.id).await;
        assert!(matches!(final_run.status, RunStatus::Completed));

        let tree_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}/workspace/tree", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("workspace tree request");
        assert_eq!(tree_response.status(), StatusCode::OK);
        let tree: WorkspaceNode = decode_json(tree_response).await;
        assert!(workspace_contains_path(
            &tree,
            &format!("runs/{}/assistant-response.md", accepted.run.id)
        ));
        assert!(workspace_contains_path(
            &tree,
            &format!("runs/{}/report.html", accepted.run.id)
        ));

        let artifacts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}/artifacts", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("session artifacts request");
        assert_eq!(artifacts_response.status(), StatusCode::OK);
        let artifacts: Vec<ArtifactRecord> = decode_json(artifacts_response).await;
        assert!(artifacts
            .iter()
            .any(|artifact| artifact.path
                == format!("runs/{}/assistant-response.md", accepted.run.id)));
        assert!(artifacts
            .iter()
            .any(|artifact| artifact.path == format!("runs/{}/report.html", accepted.run.id)));

        let task_artifacts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/tasks/{}/artifacts", accepted.run.task_id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("task artifacts request");
        assert_eq!(task_artifacts_response.status(), StatusCode::OK);
        let task_artifacts: Vec<ArtifactRecord> = decode_json(task_artifacts_response).await;
        assert_eq!(task_artifacts.len(), artifacts.len());

        let run_artifacts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{}/artifacts", accepted.run.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("run artifacts request");
        assert_eq!(run_artifacts_response.status(), StatusCode::OK);
        let run_artifacts: Vec<ArtifactRecord> = decode_json(run_artifacts_response).await;
        assert_eq!(run_artifacts.len(), artifacts.len());

        let markdown_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/sessions/{}/workspace/raw/runs/{}/assistant-response.md",
                        session.id, accepted.run.id
                    ))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("workspace raw request");
        assert_eq!(markdown_response.status(), StatusCode::OK);
        let markdown_content_type = markdown_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .expect("markdown content type");
        assert!(markdown_content_type.starts_with("text/markdown"));
        let markdown_body = to_bytes(markdown_response.into_body())
            .await
            .expect("read markdown body");
        let markdown_text =
            String::from_utf8(markdown_body.to_vec()).expect("decode markdown artifact");
        assert!(markdown_text.contains("Assistant response"));
        assert!(markdown_text.contains("workspace artifacts"));

        let render_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/sessions/{}/workspace/render/runs/{}/assistant-response.md",
                        session.id, accepted.run.id
                    ))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("workspace render request");
        assert_eq!(render_response.status(), StatusCode::OK);
        let render_content_type = render_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .expect("render content type");
        assert!(render_content_type.starts_with("text/html"));
        let render_body = to_bytes(render_response.into_body())
            .await
            .expect("read rendered markdown body");
        let rendered_html = String::from_utf8(render_body.to_vec()).expect("decode rendered html");
        assert!(rendered_html.contains("<!doctype html>"));
        assert!(rendered_html.contains("Assistant response"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn tool_and_subagent_artifacts_surface_via_api_execution_views() {
        let _lock = lock_api_test();
        let temp_root =
            std::env::temp_dir().join(format!("asuka-tool-artifact-api-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_root).expect("create temporary workspace root");
        let _workspace_root = EnvVarGuard::set(
            "ASUKA_WORKSPACE_ROOT",
            temp_root.to_str().expect("temporary workspace root"),
        );
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![
            TestOpenRouterOutcome::Response(TestOpenRouterResponse::json(
                200,
                r##"{"choices":[{"message":{"content":"{\"type\":\"tool\",\"tool\":\"write_file\",\"arguments\":{\"path\":\"notes/tool-output.md\",\"content\":\"# Tool Artifact\\n\\nCreated while the run is still executing.\\n\"}}"}}]}"##,
            )),
            TestOpenRouterOutcome::Response(TestOpenRouterResponse::json(
                200,
                r##"{"choices":[{"message":{"content":"{\"type\":\"final\",\"content\":\"The tool artifact and subagent output are ready.\"}"}}]}"##,
            )),
        ]);
        let app = build_test_router_with_openrouter_transport(transport.clone());

        let session = create_session(&app, "Artifact Streaming Session").await;
        let accepted = post_session_message(
            &app,
            session.id,
            "Use a subagent to analyze this request, write notes/tool-output.md with a markdown summary, then finish.",
        )
        .await;
        let final_run = wait_for_completed_run(&app, accepted.run.id).await;
        assert!(matches!(final_run.status, RunStatus::Completed));

        let artifacts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}/artifacts", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("session artifacts request");
        assert_eq!(artifacts_response.status(), StatusCode::OK);
        let artifacts: Vec<ArtifactRecord> = decode_json(artifacts_response).await;

        let tool_result_artifact = artifacts
            .iter()
            .find(|artifact| {
                artifact.path.contains("/tool-invocations/")
                    && artifact.path.ends_with("/result.json")
            })
            .expect("tool result artifact");
        assert_eq!(
            tool_result_artifact.producer_kind,
            Some(ArtifactProducerKind::ToolInvocation)
        );

        let tool_snapshot_artifact = artifacts
            .iter()
            .find(|artifact| {
                artifact
                    .path
                    .ends_with("/outputs/workspace/notes/tool-output.md")
            })
            .expect("tool snapshot artifact");
        assert_eq!(
            tool_snapshot_artifact.producer_kind,
            Some(ArtifactProducerKind::ToolInvocation)
        );
        assert_eq!(
            tool_snapshot_artifact.producer_ref_id,
            tool_result_artifact.producer_ref_id
        );

        let subagent_artifact = artifacts
            .iter()
            .find(|artifact| {
                artifact.path.contains("/subagents/") && artifact.path.ends_with("/summary.md")
            })
            .expect("subagent summary artifact");
        assert_eq!(
            subagent_artifact.producer_kind,
            Some(ArtifactProducerKind::RunStep)
        );

        let tree_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}/workspace/tree", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("workspace tree request");
        assert_eq!(tree_response.status(), StatusCode::OK);
        let tree: WorkspaceNode = decode_json(tree_response).await;
        assert!(workspace_contains_path(&tree, &tool_snapshot_artifact.path));
        assert!(workspace_contains_path(&tree, &subagent_artifact.path));

        let snapshot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/sessions/{}/workspace/raw/{}",
                        session.id, tool_snapshot_artifact.path
                    ))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("snapshot workspace request");
        assert_eq!(snapshot_response.status(), StatusCode::OK);
        let snapshot_body = to_bytes(snapshot_response.into_body())
            .await
            .expect("read snapshot body");
        let snapshot_text = String::from_utf8(snapshot_body.to_vec()).expect("decode snapshot");
        assert!(snapshot_text.contains("# Tool Artifact"));
        assert!(snapshot_text.contains("still executing"));

        let execution_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/tasks/{}/execution", accepted.run.task_id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("task execution request");
        assert_eq!(execution_response.status(), StatusCode::OK);
        let execution: TaskExecutionDetail = decode_json(execution_response).await;
        assert!(execution.lineage_edges.iter().any(|edge| {
            edge.relation == "produces"
                && edge.from
                    == format!(
                        "tool:{}",
                        tool_snapshot_artifact
                            .producer_ref_id
                            .expect("tool snapshot producer ref id")
                    )
                && edge.to == format!("artifact:{}", tool_snapshot_artifact.id)
        }));
        assert!(execution.lineage_edges.iter().any(|edge| {
            edge.relation == "produces"
                && edge.from.starts_with("run-step:")
                && edge.to == format!("artifact:{}", subagent_artifact.id)
        }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_events_stream_returns_sse_frames_until_completion() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;

        let session = create_session(&app, "Event Stream Session").await;

        let post_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{}/messages", session.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "content": "remember this architecture note and analyze it with a tool plus a subagent so the stream coverage test can observe several events before the run finishes. this prompt is intentionally long enough to trigger the prototype subagent path."
                        }))
                        .expect("serialize message payload"),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("post message request");

        let accepted: RunAccepted = decode_json(post_response).await;
        let stream_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{}/events", accepted.run.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("stream run events request");

        assert_eq!(stream_response.status(), StatusCode::OK);
        let content_type = stream_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .expect("sse content type header");
        assert!(content_type.starts_with("text/event-stream"));

        let events = decode_sse_until(stream_response, "run.completed").await;
        let first = events.first().expect("stream ready event");
        assert_eq!(first.event_type, "run.stream.ready");
        assert_eq!(first.data["eventType"], "run.stream.ready");
        assert_eq!(first.data["runId"], accepted.run.id.to_string());
        assert_eq!(first.data["sessionId"], session.id.to_string());

        let sequences: Vec<u64> = events
            .iter()
            .map(|event| {
                event.data["sequence"]
                    .as_u64()
                    .expect("event sequence in envelope")
            })
            .collect();
        assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(events
            .iter()
            .any(|event| event.event_type == "memory.retrieved"));
        assert!(events
            .iter()
            .any(|event| event.event_type == "message.delta"));

        let completed = events
            .iter()
            .find(|event| event.event_type == "run.completed")
            .expect("completed event");
        assert_eq!(completed.data["payload"]["status"], "completed");
        assert!(completed.data["payload"]["messageId"].as_str().is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_events_stream_includes_model_fallback_before_completion() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Error(
            "simulated upstream failure".to_string(),
        )]);
        let app = build_test_router_with_openrouter_transport(transport.clone());
        let session = create_session(&app, "Fallback Event Stream Session").await;

        let accepted = post_session_message(
            &app,
            session.id,
            "Please analyze this with a tool and a subagent so the fallback SSE test has time to connect before the provider path fails and the local response path completes. This message is intentionally long enough to trigger the prototype subagent flow.",
        )
        .await;

        let stream_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{}/events", accepted.run.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("stream run events request");

        assert_eq!(stream_response.status(), StatusCode::OK);
        let content_type = stream_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .expect("sse content type header");
        assert!(content_type.starts_with("text/event-stream"));

        let events = decode_sse_until(stream_response, "run.completed").await;
        assert_eq!(
            events.first().expect("stream ready event").event_type,
            "run.stream.ready"
        );
        assert!(events
            .iter()
            .any(|event| event.event_type == "subagent.started"));
        assert!(events
            .iter()
            .any(|event| event.event_type == "message.delta"));
        assert!(!events.iter().any(|event| event.event_type == "run.failed"));

        let fallback_event = events
            .iter()
            .find(|event| {
                event.event_type == "run.step.started"
                    && event.data["payload"]["stepType"] == "model-fallback"
            })
            .expect("model fallback event");
        assert!(fallback_event.data["payload"]["message"]
            .as_str()
            .expect("fallback message")
            .contains("simulated upstream failure"));

        let fallback_index = events
            .iter()
            .position(|event| {
                event.event_type == "run.step.started"
                    && event.data["payload"]["stepType"] == "model-fallback"
            })
            .expect("model fallback index");
        let completed_index = events
            .iter()
            .position(|event| event.event_type == "run.completed")
            .expect("completed index");
        assert!(fallback_index < completed_index);

        let recorded_requests = transport.recorded_requests();
        assert_eq!(recorded_requests.len(), 1);
        assert!(recorded_requests[0].endpoint.ends_with("/chat/completions"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn active_run_and_event_history_routes_support_stream_recovery() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let long_reply = "Deterministic streamed provider reply for recovery coverage. ".repeat(24);
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Response(
            TestOpenRouterResponse::json(
                200,
                &format!(
                    r#"{{"choices":[{{"message":{{"content":"{}"}}}}]}}"#,
                    long_reply
                ),
            ),
        )]);
        let app = build_test_router_with_openrouter_transport(transport);
        let session = create_session(&app, "Recovery Session").await;
        let accepted = post_session_message(
            &app,
            session.id,
            "Return a long deterministic provider answer for reconnect coverage.",
        )
        .await;

        let active_run = loop {
            let active = get_active_run_detail(&app, session.id).await;
            if active.run.is_some() {
                break active;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert_eq!(active_run.run.expect("active run").id, accepted.run.id);

        let history = loop {
            let history = get_run_event_history(&app, accepted.run.id, None).await;
            if history
                .events
                .iter()
                .any(|event| event.event_type == "message.delta")
            {
                break history;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert_eq!(history.run_id, accepted.run.id);
        assert!(history.last_sequence > 0);
        let last_sequence = history.last_sequence;

        let replay = get_run_event_history(&app, accepted.run.id, Some(last_sequence - 1)).await;
        assert!(replay
            .events
            .iter()
            .all(|event| event.sequence > last_sequence - 1));
        assert_eq!(
            replay.events.first().expect("replayed event").sequence,
            last_sequence
        );

        let stream_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/runs/{}/events?afterSequence={}",
                        accepted.run.id, last_sequence
                    ))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("stream replay request");

        assert_eq!(stream_response.status(), StatusCode::OK);
        let events = decode_sse_until(stream_response, "run.completed").await;
        assert_eq!(
            events.first().expect("stream ready event").event_type,
            "run.stream.ready"
        );
        assert!(events
            .iter()
            .filter(|event| event.event_type != "run.stream.ready")
            .all(|event| event.data["sequence"].as_u64().unwrap_or_default() > last_sequence));

        let final_run = wait_for_completed_run(&app, accepted.run.id).await;
        assert!(matches!(final_run.status, RunStatus::Completed));

        let active_after_completion = get_active_run_detail(&app, session.id).await;
        assert!(active_after_completion.run.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_run_route_returns_not_found_error_envelope() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("missing run request");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body: Value = decode_json(response).await;
        assert_eq!(body, json!({ "error": "run not found" }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancel_run_keeps_status_cancelled_and_skips_assistant_reply() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::remove("OPENROUTER_API_KEY");
        let app = build_test_router().await;
        let session = create_session(&app, "Cancel Session").await;

        let post_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{}/messages", session.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "content": "remember this architecture note and analyze it with a tool plus a subagent so the cancellation test holds the run open long enough to cancel it before completion. this prompt is intentionally long enough to trigger the prototype subagent path."
                        }))
                        .expect("serialize message payload"),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("post message request");
        let accepted: RunAccepted = decode_json(post_response).await;

        let cancel_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/runs/{}/cancel", accepted.run.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("cancel run request");

        assert_eq!(cancel_response.status(), StatusCode::OK);
        let cancelled: RunRecord = decode_json(cancel_response).await;
        assert!(matches!(cancelled.status, RunStatus::Cancelled));

        tokio::time::sleep(Duration::from_millis(1200)).await;

        let run_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{}", accepted.run.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get cancelled run request");
        let final_run: RunRecord = decode_json(run_response).await;
        assert!(matches!(final_run.status, RunStatus::Cancelled));

        let session_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get session request");
        let detail: SessionDetail = decode_json(session_response).await;
        assert_eq!(detail.messages.len(), 1);
        assert!(detail
            .messages
            .iter()
            .all(|message| !matches!(message.role, agent_core::MessageRole::Assistant)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn post_message_persists_fake_openrouter_reply_via_api() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Response(
            TestOpenRouterResponse::json(
                200,
                r#"{"choices":[{"message":{"content":"Deterministic provider reply from fake OpenRouter."}}]}"#,
            ),
        )]);
        let app = build_test_router_with_openrouter_transport(transport.clone());
        let session = create_session(&app, "Provider Session").await;

        let accepted =
            post_session_message(&app, session.id, "What model are you using for this reply?")
                .await;
        let final_run = wait_for_completed_run(&app, accepted.run.id).await;
        assert!(matches!(final_run.status, RunStatus::Completed));
        assert_eq!(final_run.selected_provider.as_deref(), Some("OpenRouter"));
        assert_eq!(final_run.selected_model.as_deref(), Some("demo-model"));

        let detail = get_session_detail(&app, session.id).await;
        let assistant_message = detail
            .messages
            .iter()
            .find(|message| matches!(message.role, agent_core::MessageRole::Assistant))
            .expect("assistant message");
        assert_eq!(
            assistant_message.content,
            "Deterministic provider reply from fake OpenRouter."
        );
        assert!(!assistant_message
            .content
            .contains("This run was processed by the decoupled agent-core runtime"));

        let recorded_requests = transport.recorded_requests();
        assert_eq!(recorded_requests.len(), 1);
        assert!(recorded_requests[0].endpoint.ends_with("/chat/completions"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn post_message_falls_back_when_fake_openrouter_errors_via_api() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Error(
            "simulated upstream failure".to_string(),
        )]);
        let app = build_test_router_with_openrouter_transport(transport.clone());
        let session = create_session(&app, "Provider Fallback Session").await;

        let accepted = post_session_message(
            &app,
            session.id,
            "Summarize the provider failure case for this run.",
        )
        .await;
        let final_run = wait_for_completed_run(&app, accepted.run.id).await;

        assert!(matches!(final_run.status, RunStatus::Completed));
        assert_eq!(final_run.selected_provider.as_deref(), Some("OpenRouter"));
        assert_eq!(final_run.selected_model.as_deref(), Some("demo-model"));

        let detail = get_session_detail(&app, session.id).await;
        let assistant_message = detail
            .messages
            .iter()
            .find(|message| matches!(message.role, agent_core::MessageRole::Assistant))
            .expect("assistant message");
        assert!(assistant_message
            .content
            .contains("Selected provider OpenRouter using model demo-model."));
        assert!(assistant_message
            .content
            .contains("You said: Summarize the provider failure case for this run."));
        assert!(assistant_message
            .content
            .contains("The runtime will fall back to this local response path whenever upstream model invocation is unavailable."));

        let recorded_requests = transport.recorded_requests();
        assert_eq!(recorded_requests.len(), 1);
        assert!(recorded_requests[0].endpoint.ends_with("/chat/completions"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn post_message_falls_back_when_fake_openrouter_payload_is_invalid_via_api() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let _openrouter = EnvVarGuard::set("OPENROUTER_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![TestOpenRouterOutcome::Response(
            TestOpenRouterResponse::json(200, r#"{"choices":[{"unexpected":"shape"}]}"#),
        )]);
        let app = build_test_router_with_openrouter_transport(transport.clone());
        let session = create_session(&app, "Invalid Provider Payload Session").await;

        let accepted = post_session_message(
            &app,
            session.id,
            "Summarize the invalid provider payload case for this run.",
        )
        .await;
        let final_run = wait_for_completed_run(&app, accepted.run.id).await;

        assert!(matches!(final_run.status, RunStatus::Completed));
        assert_eq!(final_run.selected_provider.as_deref(), Some("OpenRouter"));
        assert_eq!(final_run.selected_model.as_deref(), Some("demo-model"));

        let detail = get_session_detail(&app, session.id).await;
        let assistant_message = detail
            .messages
            .iter()
            .find(|message| matches!(message.role, agent_core::MessageRole::Assistant))
            .expect("assistant message");
        assert!(assistant_message
            .content
            .contains("Selected provider OpenRouter using model demo-model."));
        assert!(assistant_message
            .content
            .contains("You said: Summarize the invalid provider payload case for this run."));
        assert!(assistant_message
            .content
            .contains("The runtime will fall back to this local response path whenever upstream model invocation is unavailable."));

        let recorded_requests = transport.recorded_requests();
        assert_eq!(recorded_requests.len(), 1);
        assert!(recorded_requests[0].endpoint.ends_with("/chat/completions"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn session_skill_routes_return_effective_policy_and_overrides() {
        let _lock = lock_api_test();
        let _store = EnvVarGuard::set("AGENT_STORE", "memory");
        let app = build_test_router().await;
        let session = create_session(&app, "Skill Session").await;

        let presets_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/skill-presets")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("list skill presets request");
        assert_eq!(presets_response.status(), StatusCode::OK);
        let presets: Vec<SkillPreset> = decode_json(presets_response).await;
        assert!(presets.iter().any(|preset| preset.id == "coding"));

        let detail_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}/skills", session.id))
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("get session skills request");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail: SessionSkillsDetail = decode_json(detail_response).await;
        assert_eq!(detail.policy.mode, SessionSkillPolicyMode::InheritDefault);
        assert!(!detail.effective_skills.is_empty());

        let first_skill = detail
            .effective_skills
            .first()
            .expect("first effective skill")
            .skill
            .id;

        let patch_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/api/v1/sessions/{}/skills/{}",
                        session.id, first_skill
                    ))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "availability": "pinned"
                        }))
                        .expect("serialize binding payload"),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("patch session skill binding request");
        assert_eq!(patch_response.status(), StatusCode::OK);
        let patched: SessionSkillsDetail = decode_json(patch_response).await;
        assert!(patched
            .effective_skills
            .iter()
            .any(|entry| entry.skill.id == first_skill
                && entry.availability == SessionSkillAvailability::Pinned));

        let apply_preset_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/sessions/{}/skills/apply-preset",
                        session.id
                    ))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "presetId": "minimal"
                        }))
                        .expect("serialize preset payload"),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("apply preset request");
        assert_eq!(apply_preset_response.status(), StatusCode::OK);
        let preset_detail: SessionSkillsDetail = decode_json(apply_preset_response).await;
        assert_eq!(preset_detail.policy.mode, SessionSkillPolicyMode::Preset);
        assert_eq!(preset_detail.policy.preset_id.as_deref(), Some("minimal"));
        assert!(preset_detail.bindings.is_empty());

        let session_detail = get_session_detail(&app, session.id).await;
        assert_eq!(
            session_detail.skill_summary.policy.mode,
            SessionSkillPolicyMode::Preset
        );
        assert_eq!(
            session_detail.skill_summary.policy.preset_id.as_deref(),
            Some("minimal")
        );
    }
}
