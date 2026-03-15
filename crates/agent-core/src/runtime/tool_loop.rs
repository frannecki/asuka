use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    core::AgentCore,
    domain::{MemorySearchHit, MessageRecord, PlanStepKind},
    error::{CoreError, CoreResult},
    memory::summarize_text,
    runtime::ProviderSelection,
};

const MAX_TOOL_STEPS: usize = 8;
const MAX_TOOL_TRANSCRIPT_CHARS: usize = 10_000;

#[derive(Debug)]
enum ToolLoopAction {
    Final { content: String },
    Tool { tool_name: String, arguments: Value },
}

#[derive(Debug)]
struct ToolLoopEntry {
    tool_name: String,
    arguments: Value,
    result: Value,
}

impl AgentCore {
    pub(crate) async fn run_tool_loop(
        &self,
        selection: &ProviderSelection,
        recent_messages: &[MessageRecord],
        memory_hits: &[MemorySearchHit],
        effective_skill_names: &[String],
        pinned_skill_names: &[String],
        user_content: &str,
        session_id: Uuid,
        run_id: Uuid,
        providers_count: usize,
    ) -> CoreResult<String> {
        let tool_descriptors = self.tool_registry.descriptors();
        let mut transcript = Vec::new();

        for step in 0..MAX_TOOL_STEPS {
            let loop_prompt = build_tool_loop_prompt(
                user_content,
                &tool_descriptors,
                &transcript,
                step + 1,
                MAX_TOOL_STEPS,
            );

            let raw = self
                .generate_response(
                    Some(selection),
                    recent_messages,
                    memory_hits,
                    effective_skill_names,
                    pinned_skill_names,
                    &loop_prompt,
                    providers_count,
                )
                .await?;

            let action = parse_tool_loop_action(&raw).unwrap_or_else(|_| ToolLoopAction::Final {
                content: raw.trim().to_string(),
            });

            match action {
                ToolLoopAction::Final { content } => return Ok(content),
                ToolLoopAction::Tool {
                    tool_name,
                    arguments,
                } => {
                    let tool_step = self
                        .store
                        .start_run_step(
                            run_id,
                            None,
                            PlanStepKind::Tool,
                            format!("Invoke tool {tool_name}"),
                            summarize_text(&arguments.to_string(), 24),
                        )
                        .await?;
                    self.publish_event(
                        "tool.call.started",
                        run_id,
                        session_id,
                        json!({
                            "toolName": tool_name,
                            "arguments": arguments
                        }),
                    )
                    .await;

                    let (result_json, ok, error, extra_artifacts) = match self
                        .tool_registry
                        .execute(session_id, &tool_name, arguments.clone())
                        .await
                    {
                        Ok(result) => (
                            json!({
                                "ok": result.ok,
                                "payload": result.payload
                            }),
                            true,
                            None,
                            result.artifacts,
                        ),
                        Err(error) => (
                            json!({
                                "ok": false,
                                "error": error.message
                            }),
                            false,
                            Some(error.message),
                            Vec::new(),
                        ),
                    };
                    let invocation = self
                        .store
                        .record_tool_invocation(
                            tool_step.id,
                            tool_name.clone(),
                            "local".to_string(),
                            arguments.clone(),
                            result_json.clone(),
                            ok,
                            error.clone(),
                        )
                        .await?;
                    self.write_tool_invocation_artifacts(
                        session_id,
                        tool_step.task_id,
                        run_id,
                        &invocation,
                        &extra_artifacts,
                    )
                    .await?;
                    if ok {
                        self.store
                            .complete_run_step(
                                tool_step.id,
                                summarize_text(&result_json.to_string(), 24),
                            )
                            .await?;
                    } else {
                        self.store
                            .fail_run_step(
                                tool_step.id,
                                error
                                    .clone()
                                    .unwrap_or_else(|| "tool execution failed".to_string()),
                            )
                            .await?;
                    }

                    self.publish_event(
                        "tool.call.completed",
                        run_id,
                        session_id,
                        json!({
                            "toolName": tool_name,
                            "result": result_json
                        }),
                    )
                    .await;

                    transcript.push(ToolLoopEntry {
                        tool_name,
                        arguments,
                        result: result_json,
                    });
                }
            }
        }

        Err(CoreError::bad_request(format!(
            "tool loop exceeded max step budget of {MAX_TOOL_STEPS}"
        )))
    }
}

fn build_tool_loop_prompt(
    user_content: &str,
    tool_descriptors: &[crate::tools::ToolDescriptor],
    transcript: &[ToolLoopEntry],
    step: usize,
    max_steps: usize,
) -> String {
    let available_tools =
        serde_json::to_string_pretty(tool_descriptors).unwrap_or_else(|_| "[]".to_string());
    let transcript_json = if transcript.is_empty() {
        "[]".to_string()
    } else {
        let serialized = serde_json::to_string_pretty(
            &transcript
                .iter()
                .map(|entry| {
                    json!({
                        "toolName": entry.tool_name,
                        "arguments": entry.arguments,
                        "result": entry.result
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".to_string());

        truncate_middle(&serialized, MAX_TOOL_TRANSCRIPT_CHARS)
    };

    format!(
        "You are operating a local agent workspace with executable tools.\n\
         You must return exactly one JSON object and nothing else.\n\
         Use this schema:\n\
         {{\"type\":\"tool\",\"tool\":\"<tool name>\",\"arguments\":{{...}}}}\n\
         or\n\
         {{\"type\":\"final\",\"content\":\"<final user-facing answer>\"}}\n\n\
         Rules:\n\
         - Use tools whenever the request depends on workspace state, files, search results, or todos.\n\
         - Prefer list/read/search before write operations unless the user directly asked you to write.\n\
         - Never invent tool results.\n\
         - Keep file paths relative to the workspace root.\n\
         - If prior tool results are sufficient, return type=final.\n\
         - Current step budget: {step}/{max_steps}.\n\n\
         Available tools:\n{available_tools}\n\n\
         Completed tool transcript:\n{transcript_json}\n\n\
         Original user request:\n{user_content}\n"
    )
}

fn parse_tool_loop_action(raw: &str) -> CoreResult<ToolLoopAction> {
    let cleaned = strip_code_fences(raw).trim();
    let candidate = extract_json_object(cleaned).unwrap_or(cleaned);
    let payload = serde_json::from_str::<Value>(candidate).map_err(|error| {
        CoreError::bad_request(format!(
            "model did not return valid tool-loop JSON: {error}"
        ))
    })?;

    let action_type = payload
        .get("type")
        .or_else(|| payload.get("action"))
        .and_then(Value::as_str)
        .unwrap_or("final");

    match action_type {
        "tool" => {
            let tool_name = payload
                .get("tool")
                .or_else(|| payload.get("toolName"))
                .and_then(Value::as_str)
                .ok_or_else(|| CoreError::bad_request("tool action is missing 'tool'"))?;
            let arguments = payload
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            Ok(ToolLoopAction::Tool {
                tool_name: tool_name.to_string(),
                arguments,
            })
        }
        _ => {
            let content = payload
                .get("content")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| cleaned.to_string());
            Ok(ToolLoopAction::Final { content })
        }
    }
}

fn strip_code_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("```") {
        if let Some(end) = stripped.rfind("```") {
            let inner = &stripped[..end];
            return inner
                .strip_prefix("json")
                .map(str::trim_start)
                .unwrap_or(inner)
                .trim();
        }
    }

    trimmed
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    raw.get(start..=end)
}

fn truncate_middle(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }

    let keep = max_chars / 2;
    let start = input.chars().take(keep).collect::<String>();
    let end = input
        .chars()
        .rev()
        .take(keep)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{start}\n... truncated ...\n{end}")
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc};

    use serde_json::Value;

    use crate::domain::{CreateSessionRequest, PlanStepKind, PostMessageRequest, RunStepStatus};
    use crate::test_support::{
        create_test_core_with_openrouter_transport, moonshot_provider_config_toml,
        runtime_test_lock, EnvVarGuard, TestOpenRouterOutcome, TestOpenRouterResponse,
        TestOpenRouterTransport,
    };

    #[test]
    fn parse_tool_loop_action_accepts_tool_and_final_json() {
        match super::parse_tool_loop_action(
            r#"{"type":"tool","tool":"read_file","arguments":{"path":"README.md"}}"#,
        )
        .expect("parse tool action")
        {
            super::ToolLoopAction::Tool {
                tool_name,
                arguments,
            } => {
                assert_eq!(tool_name, "read_file");
                assert_eq!(arguments["path"], "README.md");
            }
            _ => panic!("expected tool action"),
        }

        match super::parse_tool_loop_action(r#"{"type":"final","content":"done"}"#)
            .expect("parse final action")
        {
            super::ToolLoopAction::Final { content } => assert_eq!(content, "done"),
            _ => panic!("expected final action"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_tool_loop_executes_local_tools_before_final_answer() {
        let _lock = runtime_test_lock().lock().expect("lock runtime test");
        let temp_root =
            std::env::temp_dir().join(format!("asuka-tool-loop-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_root).expect("create temp workspace");
        fs::write(temp_root.join("note.txt"), "tool loop smoke\n").expect("seed file");

        let _workspace_root = EnvVarGuard::set(
            "ASUKA_WORKSPACE_ROOT",
            temp_root.to_str().expect("temp root path"),
        );
        let _moonshot_key = EnvVarGuard::set("MOONSHOT_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![
            TestOpenRouterOutcome::Response(TestOpenRouterResponse::json(
                200,
                r#"{"choices":[{"message":{"content":"{\"type\":\"tool\",\"tool\":\"read_file\",\"arguments\":{\"path\":\"note.txt\"}}"}}]}"#,
            )),
            TestOpenRouterOutcome::Response(TestOpenRouterResponse::json(
                200,
                r#"{"choices":[{"message":{"content":"{\"type\":\"final\",\"content\":\"done after tool\"}"}}]}"#,
            )),
        ]);
        let core = create_test_core_with_openrouter_transport(
            moonshot_provider_config_toml(),
            Arc::clone(&transport),
        );

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");
        let session = core
            .create_session(CreateSessionRequest {
                title: Some("Tool Loop Test".to_string()),
            })
            .await
            .expect("create session");
        let accepted = core
            .store
            .enqueue_user_message(
                session.id,
                PostMessageRequest {
                    content: "Read note.txt and finish.".to_string(),
                },
            )
            .await
            .expect("enqueue message");

        let response = core
            .run_tool_loop(
                &selection,
                &[],
                &[],
                &[],
                &[],
                "Read note.txt and finish.",
                session.id,
                accepted.run.id,
                providers.len(),
            )
            .await
            .expect("tool loop response");

        assert_eq!(response, "done after tool");
        let requests = transport.recorded_requests();
        assert_eq!(requests.len(), 2);
        let second_request: Value =
            serde_json::from_slice(&requests[1].body).expect("decode second request");
        let prompt = second_request["messages"]
            .as_array()
            .expect("request messages")
            .iter()
            .filter_map(|message| message["content"].as_str())
            .find(|content| content.contains("\"toolName\": \"read_file\""))
            .expect("tool loop prompt");
        assert!(prompt.contains("\"toolName\": \"read_file\""));
        assert!(prompt.contains("\"content\": \"tool loop smoke"));

        let run_steps = core
            .list_run_steps(accepted.run.id)
            .await
            .expect("list run steps");
        assert!(run_steps.iter().any(|step| {
            step.kind == PlanStepKind::Tool && matches!(step.status, RunStepStatus::Completed)
        }));

        let invocations = core
            .list_tool_invocations(accepted.run.id)
            .await
            .expect("list tool invocations");
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].tool_name, "read_file");
        assert_eq!(invocations[0].arguments_json["path"], "note.txt");
        assert_eq!(invocations[0].result_json["ok"], true);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_tool_loop_persists_tool_artifacts_into_session_workspace() {
        let _lock = runtime_test_lock().lock().expect("lock runtime test");
        let temp_root =
            std::env::temp_dir().join(format!("asuka-tool-artifacts-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_root).expect("create temp workspace");

        let _workspace_root = EnvVarGuard::set(
            "ASUKA_WORKSPACE_ROOT",
            temp_root.to_str().expect("temp root path"),
        );
        let _moonshot_key = EnvVarGuard::set("MOONSHOT_API_KEY", "test-key");
        let transport = TestOpenRouterTransport::new(vec![
            TestOpenRouterOutcome::Response(TestOpenRouterResponse::json(
                200,
                r##"{"choices":[{"message":{"content":"{\"type\":\"tool\",\"tool\":\"write_file\",\"arguments\":{\"path\":\"notes/tool-output.md\",\"content\":\"# Tool Output\\n\\nPersisted during tool execution.\\n\"}}"}}]}"##,
            )),
            TestOpenRouterOutcome::Response(TestOpenRouterResponse::json(
                200,
                r##"{"choices":[{"message":{"content":"{\"type\":\"final\",\"content\":\"done after write\"}"}}]}"##,
            )),
        ]);
        let core = create_test_core_with_openrouter_transport(
            moonshot_provider_config_toml(),
            Arc::clone(&transport),
        );

        let providers = core.list_providers().await.expect("list providers");
        let selection = core
            .select_provider_model(&providers)
            .expect("select provider");
        let session = core
            .create_session(CreateSessionRequest {
                title: Some("Tool Artifact Test".to_string()),
            })
            .await
            .expect("create session");
        let accepted = core
            .store
            .enqueue_user_message(
                session.id,
                PostMessageRequest {
                    content: "Write a markdown note and finish.".to_string(),
                },
            )
            .await
            .expect("enqueue message");

        let response = core
            .run_tool_loop(
                &selection,
                &[],
                &[],
                &[],
                &[],
                "Write a markdown note and finish.",
                session.id,
                accepted.run.id,
                providers.len(),
            )
            .await
            .expect("tool loop response");

        assert_eq!(response, "done after write");
        let artifacts = core
            .list_run_artifacts(accepted.run.id)
            .await
            .expect("list run artifacts");
        let result_artifact = artifacts
            .iter()
            .find(|artifact| artifact.path.ends_with("/result.json"))
            .expect("tool result artifact");
        let snapshot_artifact = artifacts
            .iter()
            .find(|artifact| {
                artifact
                    .path
                    .ends_with("/outputs/workspace/notes/tool-output.md")
            })
            .expect("workspace snapshot artifact");
        assert_eq!(
            snapshot_artifact.producer_ref_id,
            result_artifact.producer_ref_id
        );

        let snapshot = core
            .read_session_workspace_file(session.id, &snapshot_artifact.path)
            .await
            .expect("read snapshot artifact");
        let snapshot_text = String::from_utf8(snapshot).expect("decode snapshot artifact");
        assert!(snapshot_text.contains("# Tool Output"));
        assert!(snapshot_text.contains("Persisted during tool execution."));

        let result = core
            .read_session_workspace_file(session.id, &result_artifact.path)
            .await
            .expect("read result artifact");
        let result_text = String::from_utf8(result).expect("decode result artifact");
        assert!(result_text.contains("\"toolName\": \"write_file\""));
        assert!(result_text.contains("\"path\": \"notes/tool-output.md\""));
    }
}
