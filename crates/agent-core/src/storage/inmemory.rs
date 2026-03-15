use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    config::ModelsConfig,
    domain::*,
    error::{CoreError, CoreResult},
    memory::{
        chunk_memory_document, merge_memory_hits, search_memory_hits, summarize_text, MemoryCorpus,
    },
    storage::{AgentStore, RunContext, StoreState},
};

pub struct InMemoryStore {
    state: RwLock<StoreState>,
}

impl InMemoryStore {
    pub(crate) fn seeded(config: &ModelsConfig) -> Self {
        Self {
            state: RwLock::new(StoreState::seeded(config)),
        }
    }
}

#[async_trait]
impl AgentStore for InMemoryStore {
    async fn list_sessions(&self) -> CoreResult<Vec<SessionRecord>> {
        let state = self.state.read().await;
        let mut sessions = state.sessions.values().cloned().collect::<Vec<_>>();
        sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
        Ok(sessions)
    }

    async fn create_session(&self, payload: CreateSessionRequest) -> CoreResult<SessionRecord> {
        let session = SessionRecord {
            id: Uuid::new_v4(),
            title: payload
                .title
                .unwrap_or_else(|| format!("Session {}", Utc::now().format("%H:%M:%S"))),
            status: SessionStatus::Active,
            root_agent_id: "default-root-agent".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_run_at: None,
            summary: "New session".to_string(),
        };

        let mut state = self.state.write().await;
        state.messages.insert(session.id, Vec::new());
        state
            .session_skill_policies
            .insert(session.id, SessionSkillPolicy::default_for(session.id));
        state.sessions.insert(session.id, session.clone());
        Ok(session)
    }

    async fn get_session(&self, session_id: Uuid) -> CoreResult<SessionDetail> {
        let state = self.state.read().await;
        let session = state
            .sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("session"))?;
        let messages = state.messages.get(&session_id).cloned().unwrap_or_default();
        let detail = resolve_session_skills(
            session_id,
            state.session_skill_policies.get(&session_id).cloned(),
            state
                .session_skill_bindings
                .get(&session_id)
                .cloned()
                .unwrap_or_default(),
            state.skills.values().cloned().collect(),
        );
        let mut session_tasks = state
            .tasks
            .values()
            .filter(|task| task.session_id == session_id)
            .cloned()
            .collect::<Vec<_>>();
        session_tasks.sort_by_key(|task| std::cmp::Reverse(task.updated_at));
        let latest_run_summary = state
            .runs
            .values()
            .filter(|run| run.session_id == session_id)
            .max_by_key(|run| run.started_at)
            .cloned();
        let active_run_summary = state
            .runs
            .values()
            .filter(|run| run.session_id == session_id && matches!(run.status, RunStatus::Running))
            .max_by_key(|run| run.started_at)
            .cloned();
        let latest_stream_checkpoint_summary = latest_run_summary.as_ref().and_then(|run| {
            state
                .run_events
                .get(&run.id)
                .and_then(|events| build_stream_checkpoint_summary(run, events))
        });
        Ok(SessionDetail {
            session,
            messages,
            skill_summary: summarize_session_skills(&detail),
            active_run_summary,
            latest_run_summary,
            active_task_summary: select_active_task(&session_tasks),
            latest_stream_checkpoint_summary,
        })
    }

    async fn update_session(
        &self,
        session_id: Uuid,
        payload: UpdateSessionRequest,
    ) -> CoreResult<SessionRecord> {
        let mut state = self.state.write().await;
        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| CoreError::not_found("session"))?;
        if let Some(title) = payload.title {
            session.title = title;
        }
        if let Some(status) = payload.status {
            session.status = status;
        }
        session.updated_at = Utc::now();
        Ok(session.clone())
    }

    async fn delete_session(&self, session_id: Uuid) -> CoreResult<()> {
        let mut state = self.state.write().await;
        if state.sessions.remove(&session_id).is_none() {
            return Err(CoreError::not_found("session"));
        }
        state.messages.remove(&session_id);
        state.session_skill_policies.remove(&session_id);
        state.session_skill_bindings.remove(&session_id);
        let task_ids = state
            .tasks
            .values()
            .filter(|task| task.session_id == session_id)
            .map(|task| task.id)
            .collect::<Vec<_>>();
        for task_id in task_ids {
            if let Some(task) = state.tasks.remove(&task_id) {
                if let Some(plan_id) = task.current_plan_id {
                    state.plan_steps.remove(&plan_id);
                    state.plans.remove(&plan_id);
                }
            }
        }
        let run_ids = state
            .runs
            .values()
            .filter(|run| run.session_id == session_id)
            .map(|run| run.id)
            .collect::<Vec<_>>();
        for run_id in run_ids {
            state.runs.remove(&run_id);
            state.run_events.remove(&run_id);
            state.run_steps.remove(&run_id);
            state.tool_invocations.remove(&run_id);
        }
        state
            .artifacts
            .retain(|_, artifact| artifact.session_id != session_id);
        Ok(())
    }

    async fn list_messages(&self, session_id: Uuid) -> CoreResult<Vec<MessageRecord>> {
        let state = self.state.read().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }
        Ok(state.messages.get(&session_id).cloned().unwrap_or_default())
    }

    async fn enqueue_user_message(
        &self,
        session_id: Uuid,
        payload: PostMessageRequest,
    ) -> CoreResult<RunAccepted> {
        if payload.content.trim().is_empty() {
            return Err(CoreError::bad_request("message content cannot be empty"));
        }

        let mut state = self.state.write().await;
        let skills = resolve_session_skills(
            session_id,
            state.session_skill_policies.get(&session_id).cloned(),
            state
                .session_skill_bindings
                .get(&session_id)
                .cloned()
                .unwrap_or_default(),
            state.skills.values().cloned().collect(),
        );

        let run = RunRecord {
            id: Uuid::new_v4(),
            session_id,
            task_id: Uuid::nil(),
            trigger_type: "userMessage".to_string(),
            status: RunStatus::Running,
            selected_provider: None,
            selected_model: None,
            started_at: Utc::now(),
            finished_at: None,
            error: None,
            effective_skill_names: skills
                .effective_skills
                .iter()
                .map(|entry| entry.skill.name.clone())
                .collect(),
            pinned_skill_names: skills
                .effective_skills
                .iter()
                .filter(|entry| entry.is_pinned)
                .map(|entry| entry.skill.name.clone())
                .collect(),
            last_event_sequence: 0,
            stream_status: RunStreamStatus::Active,
            active_stream_message_id: Some(Uuid::new_v4()),
        };

        let user_message = MessageRecord {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::User,
            content: payload.content.trim().to_string(),
            created_at: Utc::now(),
            run_id: Some(run.id),
        };
        let (task, plan, steps) = build_default_task_bundle(session_id, &user_message);
        let mut run = run;
        run.task_id = task.id;

        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| CoreError::not_found("session"))?;
        session.updated_at = Utc::now();
        session.last_run_at = Some(Utc::now());
        state.tasks.insert(task.id, task.clone());
        state.plans.insert(plan.id, plan.clone());
        state.plan_steps.insert(plan.id, steps);
        state.runs.insert(run.id, run.clone());
        state
            .messages
            .entry(session_id)
            .or_default()
            .push(user_message.clone());

        Ok(RunAccepted { run, user_message })
    }

    async fn get_active_run(&self, session_id: Uuid) -> CoreResult<Option<RunRecord>> {
        let state = self.state.read().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        Ok(state
            .runs
            .values()
            .filter(|run| run.session_id == session_id && matches!(run.status, RunStatus::Running))
            .max_by_key(|run| run.started_at)
            .cloned())
    }

    async fn get_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let state = self.state.read().await;
        state
            .runs
            .get(&run_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("run"))
    }

    async fn cancel_run(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let mut state = self.state.write().await;
        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.status = RunStatus::Cancelled;
        run.finished_at = Some(Utc::now());
        run.stream_status = RunStreamStatus::Cancelled;
        run.active_stream_message_id = None;
        let snapshot = run.clone();
        if let Some(task) = state.tasks.get_mut(&snapshot.task_id) {
            task.status = TaskStatus::Cancelled;
            task.updated_at = Utc::now();
            task.completed_at = Some(Utc::now());
            task.latest_run_id = Some(run_id);
        }
        Ok(snapshot)
    }

    async fn fail_run(&self, run_id: Uuid, message: String) -> CoreResult<RunRecord> {
        let mut state = self.state.write().await;
        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.status = RunStatus::Failed;
        run.finished_at = Some(Utc::now());
        run.error = Some(message);
        run.stream_status = RunStreamStatus::Failed;
        run.active_stream_message_id = None;
        let snapshot = run.clone();
        if let Some(task) = state.tasks.get_mut(&snapshot.task_id) {
            task.status = TaskStatus::Failed;
            task.updated_at = Utc::now();
            task.completed_at = Some(Utc::now());
            task.latest_run_id = Some(run_id);
        }
        Ok(snapshot)
    }

    async fn run_is_active(&self, run_id: Uuid) -> bool {
        let state = self.state.read().await;
        state
            .runs
            .get(&run_id)
            .map(|run| matches!(run.status, RunStatus::Running))
            .unwrap_or(false)
    }

    async fn list_run_events(
        &self,
        run_id: Uuid,
        after_sequence: Option<u64>,
    ) -> CoreResult<Vec<RunEventEnvelope>> {
        let state = self.state.read().await;
        if !state.runs.contains_key(&run_id) {
            return Err(CoreError::not_found("run"));
        }

        let mut events = state.run_events.get(&run_id).cloned().unwrap_or_default();
        if let Some(after_sequence) = after_sequence {
            events.retain(|event| event.sequence > after_sequence);
        }
        events.sort_by_key(|event| event.sequence);
        Ok(events)
    }

    async fn append_run_event(&self, event: RunEventEnvelope) -> CoreResult<()> {
        let mut state = self.state.write().await;
        let run_id = event.run_id;
        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.last_event_sequence = event.sequence;
        run.stream_status = match event.event_type.as_str() {
            "run.completed" => RunStreamStatus::Completed,
            "run.failed" => RunStreamStatus::Failed,
            _ if matches!(run.status, RunStatus::Cancelled) => RunStreamStatus::Cancelled,
            _ => RunStreamStatus::Active,
        };
        if matches!(event.event_type.as_str(), "run.completed" | "run.failed") {
            run.active_stream_message_id = None;
        }

        state.run_events.entry(run_id).or_default().push(event);
        if let Some(events) = state.run_events.get_mut(&run_id) {
            events.sort_by_key(|item| item.sequence);
        }
        Ok(())
    }

    async fn set_run_selection(
        &self,
        run_id: Uuid,
        provider_name: String,
        model_name: String,
    ) -> CoreResult<()> {
        let mut state = self.state.write().await;
        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.selected_provider = Some(provider_name);
        run.selected_model = Some(model_name);
        Ok(())
    }

    async fn build_run_context(
        &self,
        session_id: Uuid,
        user_content: &str,
    ) -> CoreResult<RunContext> {
        let state = self.state.read().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        let recent_messages = state
            .messages
            .get(&session_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();

        let mut providers = state.providers.values().cloned().collect::<Vec<_>>();
        providers.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        let skills = resolve_session_skills(
            session_id,
            state.session_skill_policies.get(&session_id).cloned(),
            state
                .session_skill_bindings
                .get(&session_id)
                .cloned()
                .unwrap_or_default(),
            state.skills.values().cloned().collect(),
        );

        Ok(RunContext {
            providers,
            recent_messages,
            memory_hits: merge_memory_hits(
                [
                    search_memory_hits(
                        MemoryCorpus {
                            documents: &state.memory_documents,
                            chunks: &state.memory_chunks,
                        },
                        user_content,
                        None,
                        Some(&[MemoryScope::Session]),
                        Some(session_id),
                        3,
                    ),
                    search_memory_hits(
                        MemoryCorpus {
                            documents: &state.memory_documents,
                            chunks: &state.memory_chunks,
                        },
                        user_content,
                        None,
                        Some(&[MemoryScope::Project, MemoryScope::Global]),
                        None,
                        3,
                    ),
                ],
                6,
            ),
            effective_skill_names: skills
                .effective_skills
                .iter()
                .map(|entry| entry.skill.name.clone())
                .collect(),
            pinned_skill_names: skills
                .effective_skills
                .iter()
                .filter(|entry| entry.is_pinned)
                .map(|entry| entry.skill.name.clone())
                .collect(),
        })
    }

    async fn append_assistant_message_and_complete_run(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        response: String,
    ) -> CoreResult<MessageRecord> {
        let draft_message_id = self
            .state
            .read()
            .await
            .runs
            .get(&run_id)
            .and_then(|run| run.active_stream_message_id)
            .unwrap_or_else(Uuid::new_v4);
        let assistant_message = MessageRecord {
            id: draft_message_id,
            session_id,
            role: MessageRole::Assistant,
            content: response,
            created_at: Utc::now(),
            run_id: Some(run_id),
        };

        let mut state = self.state.write().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        let run = state
            .runs
            .get(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        if !matches!(run.status, RunStatus::Running) {
            return Err(CoreError::conflict("run is no longer active"));
        }

        state
            .messages
            .entry(session_id)
            .or_default()
            .push(assistant_message.clone());

        let run = state
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        run.status = RunStatus::Completed;
        run.finished_at = Some(Utc::now());
        run.error = None;
        run.stream_status = RunStreamStatus::Completed;
        run.active_stream_message_id = None;
        let task_id = run.task_id;

        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.updated_at = Utc::now();
            session.summary = "Last run completed through the agent-core runtime.".to_string();
        }
        if let Some(task) = state.tasks.get_mut(&task_id) {
            task.status = TaskStatus::Completed;
            task.updated_at = Utc::now();
            task.completed_at = Some(Utc::now());
            task.summary = summarize_text(&assistant_message.content, 24);
            task.latest_run_id = Some(run_id);
        }

        Ok(assistant_message)
    }

    async fn list_tasks(&self, session_id: Option<Uuid>) -> CoreResult<Vec<TaskRecord>> {
        let state = self.state.read().await;
        let mut tasks = state
            .tasks
            .values()
            .filter(|task| {
                session_id
                    .map(|value| task.session_id == value)
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by_key(|task| std::cmp::Reverse(task.updated_at));
        Ok(tasks)
    }

    async fn get_task(&self, task_id: Uuid) -> CoreResult<TaskRecord> {
        let state = self.state.read().await;
        state
            .tasks
            .get(&task_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("task"))
    }

    async fn get_task_plan(&self, task_id: Uuid) -> CoreResult<PlanDetail> {
        let state = self.state.read().await;
        let task = state
            .tasks
            .get(&task_id)
            .ok_or_else(|| CoreError::not_found("task"))?;
        let plan_id = task
            .current_plan_id
            .ok_or_else(|| CoreError::not_found("task plan"))?;
        let plan = state
            .plans
            .get(&plan_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("task plan"))?;
        let steps = state.plan_steps.get(&plan_id).cloned().unwrap_or_default();
        Ok(PlanDetail { plan, steps })
    }

    async fn list_task_runs(&self, task_id: Uuid) -> CoreResult<Vec<RunRecord>> {
        let state = self.state.read().await;
        if !state.tasks.contains_key(&task_id) {
            return Err(CoreError::not_found("task"));
        }
        let mut runs = state
            .runs
            .values()
            .filter(|run| run.task_id == task_id)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by_key(|run| std::cmp::Reverse(run.started_at));
        Ok(runs)
    }

    async fn list_session_artifacts(&self, session_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        let state = self.state.read().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }
        Ok(sorted_artifacts(
            state
                .artifacts
                .values()
                .filter(|artifact| artifact.session_id == session_id)
                .cloned()
                .collect(),
        ))
    }

    async fn list_task_artifacts(&self, task_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        let state = self.state.read().await;
        if !state.tasks.contains_key(&task_id) {
            return Err(CoreError::not_found("task"));
        }
        Ok(sorted_artifacts(
            state
                .artifacts
                .values()
                .filter(|artifact| artifact.task_id == task_id)
                .cloned()
                .collect(),
        ))
    }

    async fn list_run_artifacts(&self, run_id: Uuid) -> CoreResult<Vec<ArtifactRecord>> {
        let state = self.state.read().await;
        if !state.runs.contains_key(&run_id) {
            return Err(CoreError::not_found("run"));
        }
        Ok(sorted_artifacts(
            state
                .artifacts
                .values()
                .filter(|artifact| artifact.run_id == run_id)
                .cloned()
                .collect(),
        ))
    }

    async fn upsert_artifact(&self, artifact: ArtifactRecord) -> CoreResult<ArtifactRecord> {
        let mut state = self.state.write().await;
        if !state.sessions.contains_key(&artifact.session_id) {
            return Err(CoreError::not_found("session"));
        }
        if !state.tasks.contains_key(&artifact.task_id) {
            return Err(CoreError::not_found("task"));
        }
        if !state.runs.contains_key(&artifact.run_id) {
            return Err(CoreError::not_found("run"));
        }
        state.artifacts.insert(artifact.id, artifact.clone());
        Ok(artifact)
    }

    async fn list_run_steps(&self, run_id: Uuid) -> CoreResult<Vec<RunStepRecord>> {
        let state = self.state.read().await;
        if !state.runs.contains_key(&run_id) {
            return Err(CoreError::not_found("run"));
        }
        Ok(state.run_steps.get(&run_id).cloned().unwrap_or_default())
    }

    async fn start_run_step(
        &self,
        run_id: Uuid,
        plan_step_id: Option<Uuid>,
        kind: PlanStepKind,
        title: String,
        input_summary: String,
    ) -> CoreResult<RunStepRecord> {
        let mut state = self.state.write().await;
        let task_id = state
            .runs
            .get(&run_id)
            .map(|run| run.task_id)
            .ok_or_else(|| CoreError::not_found("run"))?;
        let sequence = state
            .run_steps
            .get(&run_id)
            .map(|steps| steps.len() as u32 + 1)
            .unwrap_or(1);
        let step = RunStepRecord {
            id: Uuid::new_v4(),
            run_id,
            task_id,
            plan_step_id,
            sequence,
            kind,
            title,
            status: RunStepStatus::Running,
            input_summary,
            output_summary: None,
            started_at: Utc::now(),
            finished_at: None,
            error: None,
        };
        state
            .run_steps
            .entry(run_id)
            .or_default()
            .push(step.clone());
        if let Some(plan_step_id) = step.plan_step_id {
            set_plan_step_status(&mut state, plan_step_id, PlanStepStatus::Running)?;
        }
        Ok(step)
    }

    async fn complete_run_step(
        &self,
        run_step_id: Uuid,
        output_summary: String,
    ) -> CoreResult<RunStepRecord> {
        let mut state = self.state.write().await;
        let step = find_run_step_mut(&mut state, run_step_id)?;
        step.status = RunStepStatus::Completed;
        step.finished_at = Some(Utc::now());
        step.output_summary = Some(output_summary);
        step.error = None;
        let completed = step.clone();
        if let Some(plan_step_id) = completed.plan_step_id {
            set_plan_step_status(&mut state, plan_step_id, PlanStepStatus::Completed)?;
        }
        Ok(completed)
    }

    async fn fail_run_step(&self, run_step_id: Uuid, error: String) -> CoreResult<RunStepRecord> {
        let mut state = self.state.write().await;
        let step = find_run_step_mut(&mut state, run_step_id)?;
        step.status = RunStepStatus::Failed;
        step.finished_at = Some(Utc::now());
        step.error = Some(error);
        let failed = step.clone();
        if let Some(plan_step_id) = failed.plan_step_id {
            set_plan_step_status(&mut state, plan_step_id, PlanStepStatus::Failed)?;
        }
        Ok(failed)
    }

    async fn list_tool_invocations(&self, run_id: Uuid) -> CoreResult<Vec<ToolInvocationRecord>> {
        let state = self.state.read().await;
        if !state.runs.contains_key(&run_id) {
            return Err(CoreError::not_found("run"));
        }
        Ok(state
            .tool_invocations
            .get(&run_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn record_tool_invocation(
        &self,
        run_step_id: Uuid,
        tool_name: String,
        tool_source: String,
        arguments_json: Value,
        result_json: Value,
        ok: bool,
        error: Option<String>,
    ) -> CoreResult<ToolInvocationRecord> {
        let mut state = self.state.write().await;
        let run_step = find_run_step_mut(&mut state, run_step_id)?.clone();
        let invocation = ToolInvocationRecord {
            id: Uuid::new_v4(),
            run_step_id,
            run_id: run_step.run_id,
            tool_name,
            tool_source,
            arguments_json,
            result_json,
            ok,
            started_at: Utc::now(),
            finished_at: Utc::now(),
            error,
        };
        state
            .tool_invocations
            .entry(run_step.run_id)
            .or_default()
            .push(invocation.clone());
        Ok(invocation)
    }

    async fn write_run_memory_note(
        &self,
        session_id: Uuid,
        user_content: &str,
        response: &str,
    ) -> CoreResult<MemoryDocumentRecord> {
        let memory_document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: format!("Run note {}", Utc::now().format("%Y-%m-%d %H:%M:%S")),
            namespace: "session".to_string(),
            source: "run-summary".to_string(),
            memory_scope: MemoryScope::Session,
            owner_session_id: Some(session_id),
            owner_task_id: None,
            is_pinned: false,
            content: format!("User request: {user_content}\n\nAssistant response: {response}"),
            summary: summarize_text(response, 24),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut memory_document = memory_document;
        let chunks = chunk_memory_document(&memory_document);
        memory_document.chunk_count = chunks.len();

        let mut state = self.state.write().await;
        state.memory_chunks.extend(chunks);
        state
            .memory_documents
            .insert(memory_document.id, memory_document.clone());
        Ok(memory_document)
    }

    async fn list_skills(&self) -> CoreResult<Vec<SkillRecord>> {
        let state = self.state.read().await;
        let mut skills = state.skills.values().cloned().collect::<Vec<_>>();
        skills.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(skills)
    }

    async fn create_skill(&self, payload: CreateSkillRequest) -> CoreResult<SkillRecord> {
        let skill = SkillRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            description: payload.description,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.write().await;
        state.skills.insert(skill.id, skill.clone());
        Ok(skill)
    }

    async fn update_skill(
        &self,
        skill_id: Uuid,
        payload: UpdateSkillRequest,
    ) -> CoreResult<SkillRecord> {
        let mut state = self.state.write().await;
        let skill = state
            .skills
            .get_mut(&skill_id)
            .ok_or_else(|| CoreError::not_found("skill"))?;
        if let Some(description) = payload.description {
            skill.description = description;
        }
        if let Some(status) = payload.status {
            skill.status = status;
        }
        skill.updated_at = Utc::now();
        Ok(skill.clone())
    }

    async fn list_skill_presets(&self) -> CoreResult<Vec<SkillPreset>> {
        Ok(default_skill_presets())
    }

    async fn get_session_skills(&self, session_id: Uuid) -> CoreResult<SessionSkillsDetail> {
        let state = self.state.read().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        Ok(resolve_session_skills(
            session_id,
            state.session_skill_policies.get(&session_id).cloned(),
            state
                .session_skill_bindings
                .get(&session_id)
                .cloned()
                .unwrap_or_default(),
            state.skills.values().cloned().collect(),
        ))
    }

    async fn replace_session_skills(
        &self,
        session_id: Uuid,
        detail: SessionSkillsDetail,
    ) -> CoreResult<SessionSkillsDetail> {
        let mut state = self.state.write().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        state
            .session_skill_policies
            .insert(session_id, detail.policy.clone());
        state
            .session_skill_bindings
            .insert(session_id, detail.bindings.clone());
        Ok(resolve_session_skills(
            session_id,
            state.session_skill_policies.get(&session_id).cloned(),
            state
                .session_skill_bindings
                .get(&session_id)
                .cloned()
                .unwrap_or_default(),
            state.skills.values().cloned().collect(),
        ))
    }

    async fn update_session_skill_binding(
        &self,
        session_id: Uuid,
        skill_id: Uuid,
        payload: UpdateSessionSkillBindingRequest,
    ) -> CoreResult<SessionSkillsDetail> {
        let mut state = self.state.write().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }
        if !state.skills.contains_key(&skill_id) {
            return Err(CoreError::not_found("skill"));
        }

        let bindings = state
            .session_skill_bindings
            .entry(session_id)
            .or_insert_with(Vec::new);
        if let Some(binding) = bindings
            .iter_mut()
            .find(|binding| binding.skill_id == skill_id)
        {
            binding.availability = payload.availability;
            if let Some(order_index) = payload.order_index {
                binding.order_index = order_index;
            }
            binding.notes = payload.notes;
            binding.updated_at = Utc::now();
        } else {
            bindings.push(SessionSkillBinding {
                session_id,
                skill_id,
                availability: payload.availability,
                order_index: payload.order_index.unwrap_or(bindings.len() as i32),
                notes: payload.notes,
                updated_at: Utc::now(),
            });
        }

        let bindings = bindings.clone();
        let policy = state.session_skill_policies.get(&session_id).cloned();
        let skills = state.skills.values().cloned().collect();

        Ok(resolve_session_skills(session_id, policy, bindings, skills))
    }

    async fn apply_session_skill_preset(
        &self,
        session_id: Uuid,
        preset_id: String,
    ) -> CoreResult<SessionSkillsDetail> {
        let presets = default_skill_presets();
        if !presets.iter().any(|preset| preset.id == preset_id) {
            return Err(CoreError::bad_request("unknown skill preset"));
        }

        let mut state = self.state.write().await;
        if !state.sessions.contains_key(&session_id) {
            return Err(CoreError::not_found("session"));
        }

        state.session_skill_policies.insert(
            session_id,
            SessionSkillPolicy {
                session_id,
                mode: SessionSkillPolicyMode::Preset,
                preset_id: Some(preset_id),
                updated_at: Utc::now(),
            },
        );
        state.session_skill_bindings.insert(session_id, Vec::new());

        Ok(resolve_session_skills(
            session_id,
            state.session_skill_policies.get(&session_id).cloned(),
            Vec::new(),
            state.skills.values().cloned().collect(),
        ))
    }

    async fn list_subagents(&self) -> CoreResult<Vec<SubagentRecord>> {
        let state = self.state.read().await;
        let mut subagents = state.subagents.values().cloned().collect::<Vec<_>>();
        subagents.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(subagents)
    }

    async fn create_subagent(&self, payload: CreateSubagentRequest) -> CoreResult<SubagentRecord> {
        let subagent = SubagentRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            description: payload.description,
            scope: payload.scope,
            max_steps: payload.max_steps,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.write().await;
        state.subagents.insert(subagent.id, subagent.clone());
        Ok(subagent)
    }

    async fn get_subagent(&self, subagent_id: Uuid) -> CoreResult<SubagentRecord> {
        let state = self.state.read().await;
        state
            .subagents
            .get(&subagent_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("subagent"))
    }

    async fn update_subagent(
        &self,
        subagent_id: Uuid,
        payload: UpdateSubagentRequest,
    ) -> CoreResult<SubagentRecord> {
        let mut state = self.state.write().await;
        let subagent = state
            .subagents
            .get_mut(&subagent_id)
            .ok_or_else(|| CoreError::not_found("subagent"))?;
        if let Some(description) = payload.description {
            subagent.description = description;
        }
        if let Some(scope) = payload.scope {
            subagent.scope = scope;
        }
        if let Some(max_steps) = payload.max_steps {
            subagent.max_steps = max_steps;
        }
        if let Some(status) = payload.status {
            subagent.status = status;
        }
        subagent.updated_at = Utc::now();
        Ok(subagent.clone())
    }

    async fn list_providers(&self) -> CoreResult<Vec<ProviderAccountRecord>> {
        let state = self.state.read().await;
        let mut providers = state.providers.values().cloned().collect::<Vec<_>>();
        providers.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Ok(providers)
    }

    async fn create_provider(
        &self,
        payload: CreateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        let provider = ProviderAccountRecord {
            id: Uuid::new_v4(),
            provider_type: payload.provider_type,
            display_name: payload.display_name,
            base_url: payload.base_url,
            status: ResourceStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            models: Vec::new(),
        };

        let mut state = self.state.write().await;
        state.providers.insert(provider.id, provider.clone());
        Ok(provider)
    }

    async fn get_provider(&self, provider_id: Uuid) -> CoreResult<ProviderAccountRecord> {
        let state = self.state.read().await;
        state
            .providers
            .get(&provider_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("provider"))
    }

    async fn update_provider(
        &self,
        provider_id: Uuid,
        payload: UpdateProviderRequest,
    ) -> CoreResult<ProviderAccountRecord> {
        let mut state = self.state.write().await;
        let provider = state
            .providers
            .get_mut(&provider_id)
            .ok_or_else(|| CoreError::not_found("provider"))?;
        if let Some(display_name) = payload.display_name {
            provider.display_name = display_name;
        }
        if let Some(base_url) = payload.base_url {
            provider.base_url = Some(base_url);
        }
        if let Some(status) = payload.status {
            provider.status = status;
        }
        provider.updated_at = Utc::now();
        Ok(provider.clone())
    }

    async fn list_provider_models(
        &self,
        provider_id: Uuid,
    ) -> CoreResult<Vec<ProviderModelRecord>> {
        let state = self.state.read().await;
        let provider = state
            .providers
            .get(&provider_id)
            .ok_or_else(|| CoreError::not_found("provider"))?;
        Ok(provider.models.clone())
    }

    async fn replace_provider_models(
        &self,
        provider_id: Uuid,
        base_url: Option<String>,
        models: Vec<ProviderModelRecord>,
    ) -> CoreResult<ProviderAccountRecord> {
        let mut state = self.state.write().await;
        let provider = state
            .providers
            .get_mut(&provider_id)
            .ok_or_else(|| CoreError::not_found("provider"))?;
        provider.base_url = base_url;
        provider.models = models;
        provider.updated_at = Utc::now();
        Ok(provider.clone())
    }

    async fn list_memory_documents(&self) -> CoreResult<Vec<MemoryDocumentRecord>> {
        let state = self.state.read().await;
        let mut documents = state.memory_documents.values().cloned().collect::<Vec<_>>();
        documents.sort_by_key(|document| std::cmp::Reverse(document.updated_at));
        Ok(documents)
    }

    async fn create_memory_document(
        &self,
        payload: CreateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        if payload.title.trim().is_empty() || payload.content.trim().is_empty() {
            return Err(CoreError::bad_request(
                "memory documents require both title and content",
            ));
        }

        let namespace = payload
            .namespace
            .unwrap_or_else(|| "global".to_string())
            .trim()
            .to_string();
        let source = payload
            .source
            .unwrap_or_else(|| "manual".to_string())
            .trim()
            .to_string();
        let document = MemoryDocumentRecord {
            id: Uuid::new_v4(),
            title: payload.title.trim().to_string(),
            namespace: namespace.clone(),
            source,
            memory_scope: payload
                .memory_scope
                .unwrap_or_else(|| default_memory_scope_from_namespace(&namespace)),
            owner_session_id: payload.owner_session_id,
            owner_task_id: payload.owner_task_id,
            is_pinned: payload.is_pinned.unwrap_or(false),
            content: payload.content.trim().to_string(),
            summary: summarize_text(payload.content.trim(), 20),
            chunk_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        validate_memory_document_scope(&document)?;

        let mut document = document;
        let chunks = chunk_memory_document(&document);
        document.chunk_count = chunks.len();

        let mut state = self.state.write().await;
        state.memory_chunks.extend(chunks);
        state.memory_documents.insert(document.id, document.clone());
        Ok(document)
    }

    async fn update_memory_document(
        &self,
        document_id: Uuid,
        payload: UpdateMemoryDocumentRequest,
    ) -> CoreResult<MemoryDocumentRecord> {
        let mut state = self.state.write().await;
        let mut document = state
            .memory_documents
            .get(&document_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("memory document"))?;
        if let Some(title) = payload.title {
            document.title = title.trim().to_string();
        }
        if let Some(namespace) = payload.namespace {
            document.namespace = namespace.trim().to_string();
        }
        if let Some(memory_scope) = payload.memory_scope {
            document.memory_scope = memory_scope;
        }
        if let Some(owner_session_id) = payload.owner_session_id {
            document.owner_session_id = Some(owner_session_id);
        }
        if !matches!(document.memory_scope, MemoryScope::Session) {
            document.owner_session_id = None;
        }
        if let Some(is_pinned) = payload.is_pinned {
            document.is_pinned = is_pinned;
        }
        document.updated_at = Utc::now();
        validate_memory_document_scope(&document)?;

        state
            .memory_chunks
            .retain(|chunk| chunk.document_id != document_id);
        let chunks = chunk_memory_document(&document);
        document.chunk_count = chunks.len();
        state.memory_chunks.extend(chunks);
        state.memory_documents.insert(document.id, document.clone());
        Ok(document)
    }

    async fn delete_memory_document(&self, document_id: Uuid) -> CoreResult<()> {
        let mut state = self.state.write().await;
        if state.memory_documents.remove(&document_id).is_none() {
            return Err(CoreError::not_found("memory document"));
        }
        state
            .memory_chunks
            .retain(|chunk| chunk.document_id != document_id);
        Ok(())
    }

    async fn get_memory_document(&self, document_id: Uuid) -> CoreResult<MemoryDocumentDetail> {
        let state = self.state.read().await;
        let document = state
            .memory_documents
            .get(&document_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("memory document"))?;
        let chunks = state
            .memory_chunks
            .iter()
            .filter(|chunk| chunk.document_id == document_id)
            .cloned()
            .collect::<Vec<_>>();
        Ok(MemoryDocumentDetail { document, chunks })
    }

    async fn search_memory(&self, payload: MemorySearchRequest) -> CoreResult<MemorySearchResult> {
        if payload.query.trim().is_empty() {
            return Err(CoreError::bad_request(
                "memory search query cannot be empty",
            ));
        }

        let state = self.state.read().await;
        Ok(MemorySearchResult {
            hits: search_memory_hits(
                MemoryCorpus {
                    documents: &state.memory_documents,
                    chunks: &state.memory_chunks,
                },
                payload.query.trim(),
                payload.namespace.as_deref(),
                payload.memory_scopes.as_deref(),
                payload.owner_session_id,
                payload.limit.unwrap_or(5),
            ),
        })
    }

    async fn reindex_memory(&self) -> CoreResult<ReindexResult> {
        let mut state = self.state.write().await;
        state.memory_chunks.clear();

        let mut documents = state
            .memory_documents
            .values()
            .cloned()
            .collect::<Vec<MemoryDocumentRecord>>();
        documents.sort_by_key(|document| document.created_at);

        for document in &mut documents {
            let chunks = chunk_memory_document(document);
            document.chunk_count = chunks.len();
            document.updated_at = Utc::now();
            state.memory_chunks.extend(chunks);
            state.memory_documents.insert(document.id, document.clone());
        }

        Ok(ReindexResult {
            documents: state.memory_documents.len(),
            chunks: state.memory_chunks.len(),
        })
    }

    async fn list_mcp_servers(&self) -> CoreResult<Vec<McpServerRecord>> {
        let state = self.state.read().await;
        let mut servers = state.mcp_servers.values().cloned().collect::<Vec<_>>();
        servers.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(servers)
    }

    async fn create_mcp_server(
        &self,
        payload: CreateMcpServerRequest,
    ) -> CoreResult<McpServerRecord> {
        let server = McpServerRecord {
            id: Uuid::new_v4(),
            name: payload.name,
            transport: payload.transport,
            command: payload.command,
            status: ResourceStatus::Active,
            capabilities: vec!["tools.call".into(), "resources.read".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.write().await;
        state.mcp_servers.insert(server.id, server.clone());
        Ok(server)
    }

    async fn get_mcp_server(&self, server_id: Uuid) -> CoreResult<McpServerRecord> {
        let state = self.state.read().await;
        state
            .mcp_servers
            .get(&server_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("mcp server"))
    }

    async fn test_mcp_server(&self, server_id: Uuid) -> CoreResult<TestResult> {
        let state = self.state.read().await;
        let server = state
            .mcp_servers
            .get(&server_id)
            .ok_or_else(|| CoreError::not_found("mcp server"))?;
        Ok(TestResult {
            ok: true,
            message: format!(
                "{} is reachable in this prototype via {} transport.",
                server.name, server.transport
            ),
        })
    }

    async fn get_mcp_capabilities(&self, server_id: Uuid) -> CoreResult<CapabilityEnvelope> {
        let state = self.state.read().await;
        let server = state
            .mcp_servers
            .get(&server_id)
            .ok_or_else(|| CoreError::not_found("mcp server"))?;
        Ok(CapabilityEnvelope {
            capabilities: server.capabilities.clone(),
        })
    }
}

fn default_memory_scope_from_namespace(namespace: &str) -> MemoryScope {
    match namespace {
        "session" => MemoryScope::Session,
        "global" => MemoryScope::Global,
        _ => MemoryScope::Project,
    }
}

fn validate_memory_document_scope(document: &MemoryDocumentRecord) -> CoreResult<()> {
    if matches!(document.memory_scope, MemoryScope::Session) && document.owner_session_id.is_none()
    {
        return Err(CoreError::bad_request(
            "session-scoped memory requires ownerSessionId",
        ));
    }

    Ok(())
}

fn sorted_artifacts(mut artifacts: Vec<ArtifactRecord>) -> Vec<ArtifactRecord> {
    artifacts.sort_by_key(|artifact| std::cmp::Reverse(artifact.updated_at));
    artifacts
}

fn build_default_task_bundle(
    session_id: Uuid,
    user_message: &MessageRecord,
) -> (TaskRecord, PlanRecord, Vec<PlanStepRecord>) {
    let now = Utc::now();
    let task_id = Uuid::new_v4();
    let plan_id = Uuid::new_v4();
    let title = summarize_text(&user_message.content, 8);
    let task = TaskRecord {
        id: task_id,
        session_id,
        title: if title.is_empty() {
            "Untitled task".to_string()
        } else {
            title
        },
        goal: user_message.content.clone(),
        status: TaskStatus::Running,
        kind: "chatTurn".to_string(),
        origin_message_id: user_message.id,
        current_plan_id: Some(plan_id),
        latest_run_id: user_message.run_id,
        summary: "Task created from a user message.".to_string(),
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    let plan = PlanRecord {
        id: plan_id,
        task_id,
        version: 1,
        status: PlanStatus::Active,
        source: "defaultHarnessPlanner".to_string(),
        planning_model: None,
        created_at: now,
        superseded_at: None,
    };
    let steps = vec![
        PlanStepRecord {
            id: Uuid::new_v4(),
            plan_id,
            ordinal: 1,
            title: "Build context".to_string(),
            description: "Assemble recent session state and relevant memory.".to_string(),
            kind: PlanStepKind::ContextBuild,
            status: PlanStepStatus::Pending,
            depends_on: Vec::new(),
            skill_id: None,
            subagent_id: None,
            expected_outputs: vec!["context bundle".to_string()],
            acceptance_criteria: vec!["recent messages loaded".to_string()],
        },
        PlanStepRecord {
            id: Uuid::new_v4(),
            plan_id,
            ordinal: 2,
            title: "Produce response".to_string(),
            description: "Use the selected model and tools to answer the user.".to_string(),
            kind: PlanStepKind::Respond,
            status: PlanStepStatus::Pending,
            depends_on: Vec::new(),
            skill_id: None,
            subagent_id: None,
            expected_outputs: vec!["assistant response".to_string()],
            acceptance_criteria: vec!["assistant response persisted".to_string()],
        },
    ];
    (task, plan, steps)
}

fn set_plan_step_status(
    state: &mut StoreState,
    plan_step_id: Uuid,
    status: PlanStepStatus,
) -> CoreResult<()> {
    for steps in state.plan_steps.values_mut() {
        if let Some(step) = steps.iter_mut().find(|step| step.id == plan_step_id) {
            step.status = status;
            return Ok(());
        }
    }

    Err(CoreError::not_found("plan step"))
}

fn find_run_step_mut(state: &mut StoreState, run_step_id: Uuid) -> CoreResult<&mut RunStepRecord> {
    for steps in state.run_steps.values_mut() {
        if let Some(step) = steps.iter_mut().find(|step| step.id == run_step_id) {
            return Ok(step);
        }
    }

    Err(CoreError::not_found("run step"))
}

fn select_active_task(tasks: &[TaskRecord]) -> Option<TaskRecord> {
    tasks
        .iter()
        .find(|task| {
            matches!(
                task.status,
                TaskStatus::Queued
                    | TaskStatus::Planning
                    | TaskStatus::Running
                    | TaskStatus::WaitingForApproval
                    | TaskStatus::Suspended
            )
        })
        .cloned()
        .or_else(|| tasks.first().cloned())
}

fn build_stream_checkpoint_summary(
    run: &RunRecord,
    events: &[RunEventEnvelope],
) -> Option<StreamCheckpointSummary> {
    let last_event = events.last()?;
    let draft_reply_text = events
        .iter()
        .filter(|event| event.event_type == "message.delta")
        .filter_map(|event| event.payload.get("delta").and_then(|value| value.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    Some(StreamCheckpointSummary {
        run_id: run.id,
        last_sequence: last_event.sequence,
        draft_reply_text,
        updated_at: last_event.timestamp,
        active_stream_message_id: run.active_stream_message_id,
    })
}
