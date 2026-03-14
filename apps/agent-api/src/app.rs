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
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
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
        AgentCore, RunAccepted, RunRecord, RunStatus, SessionDetail, SessionRecord,
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
}
