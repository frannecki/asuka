use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::{
    domain::*,
    error::{CoreError, CoreResult},
    memory::summarize_text,
};

use super::{
    helpers::{
        expect_changed, load_json_record, load_json_records, serialize_record, sqlite_error,
    },
    store::SqliteStore,
    tables::{agent_plan_steps, agent_plans, agent_tasks},
};

impl SqliteStore {
    pub(super) async fn list_tasks_db(
        &self,
        session_id: Option<Uuid>,
    ) -> CoreResult<Vec<TaskRecord>> {
        let mut connection = self.open_connection()?;
        match session_id {
            Some(session_id) => load_json_records(
                &mut connection,
                agent_tasks::table
                    .filter(agent_tasks::session_id.eq(session_id.to_string()))
                    .order(agent_tasks::updated_at.desc())
                    .select(agent_tasks::data),
                "task",
            ),
            None => load_json_records(
                &mut connection,
                agent_tasks::table
                    .order(agent_tasks::updated_at.desc())
                    .select(agent_tasks::data),
                "task",
            ),
        }
    }

    pub(super) async fn get_task_db(&self, task_id: Uuid) -> CoreResult<TaskRecord> {
        let mut connection = self.open_connection()?;
        load_json_record(
            &mut connection,
            agent_tasks::table
                .filter(agent_tasks::id.eq(task_id.to_string()))
                .select(agent_tasks::data),
            "task",
        )
    }

    pub(super) async fn get_task_plan_db(&self, task_id: Uuid) -> CoreResult<PlanDetail> {
        let mut connection = self.open_connection()?;
        let task = load_json_record::<TaskRecord, _>(
            &mut connection,
            agent_tasks::table
                .filter(agent_tasks::id.eq(task_id.to_string()))
                .select(agent_tasks::data),
            "task",
        )?;
        let plan_id = task
            .current_plan_id
            .ok_or_else(|| CoreError::not_found("task plan"))?;
        let plan = load_json_record::<PlanRecord, _>(
            &mut connection,
            agent_plans::table
                .filter(agent_plans::id.eq(plan_id.to_string()))
                .select(agent_plans::data),
            "task plan",
        )?;
        let steps = load_json_records(
            &mut connection,
            agent_plan_steps::table
                .filter(agent_plan_steps::plan_id.eq(plan_id.to_string()))
                .order(agent_plan_steps::ordinal.asc())
                .select(agent_plan_steps::data),
            "plan step",
        )?;
        Ok(PlanDetail { plan, steps })
    }

    pub(super) fn insert_task_bundle_sqlite(
        connection: &mut SqliteConnection,
        task: &TaskRecord,
        plan: &PlanRecord,
        steps: &[PlanStepRecord],
    ) -> CoreResult<()> {
        diesel::insert_into(agent_tasks::table)
            .values((
                agent_tasks::id.eq(task.id.to_string()),
                agent_tasks::session_id.eq(task.session_id.to_string()),
                agent_tasks::updated_at.eq(task.updated_at.to_rfc3339()),
                agent_tasks::data.eq(serialize_record(task, "task")?),
            ))
            .execute(connection)
            .map_err(|error| sqlite_error("insert task", error))?;

        diesel::insert_into(agent_plans::table)
            .values((
                agent_plans::id.eq(plan.id.to_string()),
                agent_plans::task_id.eq(plan.task_id.to_string()),
                agent_plans::version.eq(plan.version as i64),
                agent_plans::status.eq(serde_variant(&plan.status)),
                agent_plans::created_at.eq(plan.created_at.to_rfc3339()),
                agent_plans::data.eq(serialize_record(plan, "task plan")?),
            ))
            .execute(connection)
            .map_err(|error| sqlite_error("insert task plan", error))?;

        for step in steps {
            diesel::insert_into(agent_plan_steps::table)
                .values((
                    agent_plan_steps::id.eq(step.id.to_string()),
                    agent_plan_steps::plan_id.eq(step.plan_id.to_string()),
                    agent_plan_steps::ordinal.eq(step.ordinal as i64),
                    agent_plan_steps::status.eq(serde_variant(&step.status)),
                    agent_plan_steps::data.eq(serialize_record(step, "plan step")?),
                ))
                .execute(connection)
                .map_err(|error| sqlite_error("insert plan step", error))?;
        }

        Ok(())
    }

    pub(super) fn update_plan_step_status_db(
        connection: &mut SqliteConnection,
        plan_step_id: Uuid,
        status: PlanStepStatus,
    ) -> CoreResult<PlanStepRecord> {
        let mut step = load_json_record::<PlanStepRecord, _>(
            connection,
            agent_plan_steps::table
                .filter(agent_plan_steps::id.eq(plan_step_id.to_string()))
                .select(agent_plan_steps::data),
            "plan step",
        )?;
        step.status = status.clone();
        let updated = diesel::update(
            agent_plan_steps::table.filter(agent_plan_steps::id.eq(plan_step_id.to_string())),
        )
        .set((
            agent_plan_steps::status.eq(serde_variant(&status)),
            agent_plan_steps::data.eq(serialize_record(&step, "plan step")?),
        ))
        .execute(connection)
        .map_err(|error| sqlite_error("update plan step status", error))?;
        expect_changed(updated, "plan step")?;
        Ok(step)
    }

    pub(super) fn update_task_after_run_db(
        connection: &mut SqliteConnection,
        task_id: Uuid,
        run_id: Uuid,
        status: TaskStatus,
        summary: String,
    ) -> CoreResult<TaskRecord> {
        let mut task = load_json_record::<TaskRecord, _>(
            connection,
            agent_tasks::table
                .filter(agent_tasks::id.eq(task_id.to_string()))
                .select(agent_tasks::data),
            "task",
        )?;
        task.status = status;
        task.updated_at = Utc::now();
        task.latest_run_id = Some(run_id);
        if matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            task.completed_at = Some(Utc::now());
        }
        task.summary = summary;
        let updated =
            diesel::update(agent_tasks::table.filter(agent_tasks::id.eq(task.id.to_string())))
                .set((
                    agent_tasks::updated_at.eq(task.updated_at.to_rfc3339()),
                    agent_tasks::data.eq(serialize_record(&task, "task")?),
                ))
                .execute(connection)
                .map_err(|error| sqlite_error("update task", error))?;
        expect_changed(updated, "task")?;
        Ok(task)
    }
}

pub(super) fn build_default_task_bundle(
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

fn serde_variant<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|json| json.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}
