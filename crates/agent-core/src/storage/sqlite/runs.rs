use chrono::Utc;
use diesel::{
    dsl::{exists, max, select},
    prelude::*,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    domain::*,
    error::{CoreError, CoreResult},
    memory::{search_memory_hits, MemoryCorpus},
    storage::RunContext,
};

use super::{
    helpers::{
        expect_changed, load_json_record, load_json_records, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::{
        agent_messages, agent_providers, agent_run_steps, agent_runs, agent_sessions, agent_tasks,
        agent_tool_invocations,
    },
};

impl SqliteStore {
    pub(super) async fn list_task_runs_db(&self, task_id: Uuid) -> CoreResult<Vec<RunRecord>> {
        let mut connection = self.open_connection()?;
        let task_exists = select(exists(
            agent_tasks::table.filter(agent_tasks::id.eq(task_id.to_string())),
        ))
        .get_result::<bool>(&mut connection)
        .map_err(|error| sqlite_error("lookup task", error))?;
        if !task_exists {
            return Err(CoreError::not_found("task"));
        }
        load_json_records(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::task_id.eq(task_id.to_string()))
                .order(agent_runs::started_at.desc())
                .select(agent_runs::data),
            "run",
        )
    }

    pub(super) async fn get_run_db(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let mut connection = self.open_connection()?;
        load_json_record(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        )
    }

    pub(super) async fn cancel_run_db(&self, run_id: Uuid) -> CoreResult<RunRecord> {
        let mut connection = self.open_connection()?;
        let mut run = load_json_record::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        )?;
        run.status = RunStatus::Cancelled;
        run.finished_at = Some(Utc::now());
        run.stream_status = RunStreamStatus::Cancelled;
        run.active_stream_message_id = None;
        let updated =
            diesel::update(agent_runs::table.filter(agent_runs::id.eq(run.id.to_string())))
                .set((
                    agent_runs::finished_at.eq(run.finished_at.map(|value| value.to_rfc3339())),
                    agent_runs::data.eq(serialize_record(&run, "run")?),
                ))
                .execute(&mut connection)
                .map_err(|error| sqlite_error("update run", error))?;
        expect_changed(updated, "run")?;
        SqliteStore::update_task_after_run_db(
            &mut connection,
            run.task_id,
            run.id,
            TaskStatus::Cancelled,
            "Task was cancelled by the client.".to_string(),
        )?;
        Ok(run)
    }

    pub(super) async fn fail_run_db(&self, run_id: Uuid, message: String) -> CoreResult<RunRecord> {
        let mut connection = self.open_connection()?;
        let mut run = load_json_record::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        )?;
        run.status = RunStatus::Failed;
        run.finished_at = Some(Utc::now());
        run.error = Some(message);
        run.stream_status = RunStreamStatus::Failed;
        run.active_stream_message_id = None;
        let updated =
            diesel::update(agent_runs::table.filter(agent_runs::id.eq(run.id.to_string())))
                .set((
                    agent_runs::finished_at.eq(run.finished_at.map(|value| value.to_rfc3339())),
                    agent_runs::data.eq(serialize_record(&run, "run")?),
                ))
                .execute(&mut connection)
                .map_err(|error| sqlite_error("update run", error))?;
        expect_changed(updated, "run")?;
        SqliteStore::update_task_after_run_db(
            &mut connection,
            run.task_id,
            run.id,
            TaskStatus::Failed,
            run.error
                .clone()
                .unwrap_or_else(|| "Task failed.".to_string()),
        )?;
        Ok(run)
    }

    pub(super) async fn run_is_active_db(&self, run_id: Uuid) -> bool {
        let mut connection = match self.open_connection() {
            Ok(connection) => connection,
            Err(_) => return false,
        };
        match load_json_record::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        ) {
            Ok(run) => matches!(run.status, RunStatus::Running),
            Err(_) => false,
        }
    }

    pub(super) async fn set_run_selection_db(
        &self,
        run_id: Uuid,
        provider_name: String,
        model_name: String,
    ) -> CoreResult<()> {
        let mut connection = self.open_connection()?;
        let mut run = load_json_record::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        )?;
        run.selected_provider = Some(provider_name);
        run.selected_model = Some(model_name);
        let updated =
            diesel::update(agent_runs::table.filter(agent_runs::id.eq(run.id.to_string())))
                .set((
                    agent_runs::finished_at.eq(run.finished_at.map(|value| value.to_rfc3339())),
                    agent_runs::data.eq(serialize_record(&run, "run")?),
                ))
                .execute(&mut connection)
                .map_err(|error| sqlite_error("update run selection", error))?;
        expect_changed(updated, "run")
    }

    pub(super) async fn build_run_context_db(
        &self,
        session_id: Uuid,
        user_content: &str,
    ) -> CoreResult<RunContext> {
        let (mut recent_messages, providers) = {
            let mut connection = self.open_connection()?;
            let session = load_json_record::<SessionRecord, _>(
                &mut connection,
                agent_sessions::table
                    .filter(agent_sessions::id.eq(session_id.to_string()))
                    .select(agent_sessions::data),
                "session",
            )?;
            drop(session);
            let recent_messages = load_json_records(
                &mut connection,
                agent_messages::table
                    .filter(agent_messages::session_id.eq(session_id.to_string()))
                    .order(agent_messages::created_at.desc())
                    .limit(6)
                    .select(agent_messages::data),
                "message",
            )?;
            let providers = load_json_records(
                &mut connection,
                agent_providers::table
                    .order(agent_providers::updated_at.desc())
                    .select(agent_providers::data),
                "provider",
            )?;
            (recent_messages, providers)
        };
        recent_messages.reverse();
        let skills = self.get_session_skills_db(session_id).await?;

        let session_semantic_hits = self
            .search_memory_semantic(
                user_content,
                None,
                Some(&[MemoryScope::Session]),
                Some(session_id),
                3,
            )
            .await?;
        let long_term_semantic_hits = self
            .search_memory_semantic(
                user_content,
                None,
                Some(&[MemoryScope::Project, MemoryScope::Global]),
                None,
                3,
            )
            .await?;

        let memory_hits = match (session_semantic_hits, long_term_semantic_hits) {
            (Some(session_hits), Some(long_term_hits)) => {
                crate::memory::merge_memory_hits([session_hits, long_term_hits], 6)
            }
            _ => {
                let state = self.load_memory_state()?;
                crate::memory::merge_memory_hits(
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
                )
            }
        };

        Ok(RunContext {
            providers,
            recent_messages,
            memory_hits,
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

    pub(super) async fn append_assistant_message_and_complete_run_db(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        response: String,
    ) -> CoreResult<MessageRecord> {
        let mut connection = self.open_connection()?;
        connection.transaction::<_, CoreError, _>(|transaction| {
            let _session = load_json_record::<SessionRecord, _>(
                transaction,
                agent_sessions::table
                    .filter(agent_sessions::id.eq(session_id.to_string()))
                    .select(agent_sessions::data),
                "session",
            )?;
            let mut run = load_json_record::<RunRecord, _>(
                transaction,
                agent_runs::table
                    .filter(agent_runs::id.eq(run_id.to_string()))
                    .select(agent_runs::data),
                "run",
            )?;
            if !matches!(run.status, RunStatus::Running) {
                return Err(CoreError::conflict("run is no longer active"));
            }
            let assistant_message = MessageRecord {
                id: run.active_stream_message_id.unwrap_or_else(Uuid::new_v4),
                session_id,
                role: MessageRole::Assistant,
                content: response,
                created_at: Utc::now(),
                run_id: Some(run_id),
            };

            diesel::insert_into(agent_messages::table)
                .values((
                    agent_messages::id.eq(assistant_message.id.to_string()),
                    agent_messages::session_id.eq(assistant_message.session_id.to_string()),
                    agent_messages::run_id
                        .eq(assistant_message.run_id.map(|value| value.to_string())),
                    agent_messages::created_at.eq(assistant_message.created_at.to_rfc3339()),
                    agent_messages::data
                        .eq(serialize_record(&assistant_message, "assistant message")?),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("insert assistant message", error))?;

            run.status = RunStatus::Completed;
            run.finished_at = Some(Utc::now());
            run.error = None;
            run.stream_status = RunStreamStatus::Completed;
            run.active_stream_message_id = None;
            diesel::update(agent_runs::table.filter(agent_runs::id.eq(run.id.to_string())))
                .set((
                    agent_runs::finished_at.eq(run.finished_at.map(|value| value.to_rfc3339())),
                    agent_runs::data.eq(serialize_record(&run, "run")?),
                ))
                .execute(transaction)
                .map_err(|error| sqlite_error("update run during completion", error))?;

            let mut session = load_json_record::<SessionRecord, _>(
                transaction,
                agent_sessions::table
                    .filter(agent_sessions::id.eq(session_id.to_string()))
                    .select(agent_sessions::data),
                "session",
            )?;
            session.updated_at = Utc::now();
            session.summary = "Last run completed through the agent-core runtime.".to_string();
            diesel::update(
                agent_sessions::table.filter(agent_sessions::id.eq(session.id.to_string())),
            )
            .set((
                agent_sessions::updated_at.eq(session.updated_at.to_rfc3339()),
                agent_sessions::data.eq(serialize_record(&session, "session")?),
            ))
            .execute(transaction)
            .map_err(|error| sqlite_error("update session during completion", error))?;

            SqliteStore::update_task_after_run_db(
                transaction,
                run.task_id,
                run.id,
                TaskStatus::Completed,
                crate::memory::summarize_text(&assistant_message.content, 24),
            )?;

            Ok(assistant_message)
        })
    }

    pub(super) async fn list_run_steps_db(&self, run_id: Uuid) -> CoreResult<Vec<RunStepRecord>> {
        let mut connection = self.open_connection()?;
        let _run = load_json_record::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        )?;
        load_json_records(
            &mut connection,
            agent_run_steps::table
                .filter(agent_run_steps::run_id.eq(run_id.to_string()))
                .order(agent_run_steps::sequence.asc())
                .select(agent_run_steps::data),
            "run step",
        )
    }

    pub(super) async fn start_run_step_db(
        &self,
        run_id: Uuid,
        plan_step_id: Option<Uuid>,
        kind: PlanStepKind,
        title: String,
        input_summary: String,
    ) -> CoreResult<RunStepRecord> {
        let mut connection = self.open_connection()?;
        let run = load_json_record::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        )?;
        let sequence = agent_run_steps::table
            .filter(agent_run_steps::run_id.eq(run_id.to_string()))
            .select(max(agent_run_steps::sequence))
            .first::<Option<i64>>(&mut connection)
            .map_err(|error| sqlite_error("load run step sequence", error))?
            .unwrap_or(0)
            + 1;
        let step = RunStepRecord {
            id: Uuid::new_v4(),
            run_id,
            task_id: run.task_id,
            plan_step_id,
            sequence: sequence as u32,
            kind,
            title,
            status: RunStepStatus::Running,
            input_summary,
            output_summary: None,
            started_at: Utc::now(),
            finished_at: None,
            error: None,
        };
        diesel::insert_into(agent_run_steps::table)
            .values((
                agent_run_steps::id.eq(step.id.to_string()),
                agent_run_steps::run_id.eq(step.run_id.to_string()),
                agent_run_steps::task_id.eq(step.task_id.to_string()),
                agent_run_steps::sequence.eq(step.sequence as i64),
                agent_run_steps::started_at.eq(step.started_at.to_rfc3339()),
                agent_run_steps::finished_at.eq(Option::<String>::None),
                agent_run_steps::data.eq(serialize_record(&step, "run step")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert run step", error))?;
        if let Some(plan_step_id) = step.plan_step_id {
            SqliteStore::update_plan_step_status_db(
                &mut connection,
                plan_step_id,
                PlanStepStatus::Running,
            )?;
        }
        Ok(step)
    }

    pub(super) async fn complete_run_step_db(
        &self,
        run_step_id: Uuid,
        output_summary: String,
    ) -> CoreResult<RunStepRecord> {
        let mut connection = self.open_connection()?;
        let mut step = load_json_record::<RunStepRecord, _>(
            &mut connection,
            agent_run_steps::table
                .filter(agent_run_steps::id.eq(run_step_id.to_string()))
                .select(agent_run_steps::data),
            "run step",
        )?;
        step.status = RunStepStatus::Completed;
        step.finished_at = Some(Utc::now());
        step.output_summary = Some(output_summary);
        step.error = None;
        let updated = diesel::update(
            agent_run_steps::table.filter(agent_run_steps::id.eq(step.id.to_string())),
        )
        .set((
            agent_run_steps::finished_at.eq(step.finished_at.map(|value| value.to_rfc3339())),
            agent_run_steps::data.eq(serialize_record(&step, "run step")?),
        ))
        .execute(&mut connection)
        .map_err(|error| sqlite_error("update run step", error))?;
        expect_changed(updated, "run step")?;
        if let Some(plan_step_id) = step.plan_step_id {
            SqliteStore::update_plan_step_status_db(
                &mut connection,
                plan_step_id,
                PlanStepStatus::Completed,
            )?;
        }
        Ok(step)
    }

    pub(super) async fn fail_run_step_db(
        &self,
        run_step_id: Uuid,
        error: String,
    ) -> CoreResult<RunStepRecord> {
        let mut connection = self.open_connection()?;
        let mut step = load_json_record::<RunStepRecord, _>(
            &mut connection,
            agent_run_steps::table
                .filter(agent_run_steps::id.eq(run_step_id.to_string()))
                .select(agent_run_steps::data),
            "run step",
        )?;
        step.status = RunStepStatus::Failed;
        step.finished_at = Some(Utc::now());
        step.error = Some(error);
        let updated = diesel::update(
            agent_run_steps::table.filter(agent_run_steps::id.eq(step.id.to_string())),
        )
        .set((
            agent_run_steps::finished_at.eq(step.finished_at.map(|value| value.to_rfc3339())),
            agent_run_steps::data.eq(serialize_record(&step, "run step")?),
        ))
        .execute(&mut connection)
        .map_err(|error| sqlite_error("fail run step", error))?;
        expect_changed(updated, "run step")?;
        if let Some(plan_step_id) = step.plan_step_id {
            SqliteStore::update_plan_step_status_db(
                &mut connection,
                plan_step_id,
                PlanStepStatus::Failed,
            )?;
        }
        Ok(step)
    }

    pub(super) async fn list_tool_invocations_db(
        &self,
        run_id: Uuid,
    ) -> CoreResult<Vec<ToolInvocationRecord>> {
        let mut connection = self.open_connection()?;
        let _run = load_json_record::<RunRecord, _>(
            &mut connection,
            agent_runs::table
                .filter(agent_runs::id.eq(run_id.to_string()))
                .select(agent_runs::data),
            "run",
        )?;
        load_json_records(
            &mut connection,
            agent_tool_invocations::table
                .filter(agent_tool_invocations::run_id.eq(run_id.to_string()))
                .order(agent_tool_invocations::started_at.asc())
                .select(agent_tool_invocations::data),
            "tool invocation",
        )
    }

    pub(super) async fn record_tool_invocation_db(
        &self,
        run_step_id: Uuid,
        tool_name: String,
        tool_source: String,
        arguments_json: Value,
        result_json: Value,
        ok: bool,
        error: Option<String>,
    ) -> CoreResult<ToolInvocationRecord> {
        let mut connection = self.open_connection()?;
        let run_step = load_json_record::<RunStepRecord, _>(
            &mut connection,
            agent_run_steps::table
                .filter(agent_run_steps::id.eq(run_step_id.to_string()))
                .select(agent_run_steps::data),
            "run step",
        )?;
        let invocation = ToolInvocationRecord {
            id: Uuid::new_v4(),
            run_step_id,
            run_id: run_step.run_id,
            tool_name: tool_name.clone(),
            tool_source,
            arguments_json,
            result_json,
            ok,
            started_at: Utc::now(),
            finished_at: Utc::now(),
            error,
        };
        diesel::insert_into(agent_tool_invocations::table)
            .values((
                agent_tool_invocations::id.eq(invocation.id.to_string()),
                agent_tool_invocations::run_id.eq(invocation.run_id.to_string()),
                agent_tool_invocations::run_step_id.eq(invocation.run_step_id.to_string()),
                agent_tool_invocations::tool_name.eq(tool_name),
                agent_tool_invocations::started_at.eq(invocation.started_at.to_rfc3339()),
                agent_tool_invocations::finished_at.eq(invocation.finished_at.to_rfc3339()),
                agent_tool_invocations::data.eq(serialize_record(&invocation, "tool invocation")?),
            ))
            .execute(&mut connection)
            .map_err(|error| sqlite_error("insert tool invocation", error))?;
        Ok(invocation)
    }
}
