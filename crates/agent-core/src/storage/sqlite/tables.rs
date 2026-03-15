diesel::table! {
    agent_artifacts (id) {
        id -> Text,
        session_id -> Text,
        task_id -> Text,
        run_id -> Text,
        path -> Text,
        created_at -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_mcp_servers (id) {
        id -> Text,
        name -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_memory_chunks (id) {
        id -> Text,
        document_id -> Text,
        namespace -> Text,
        ordinal -> BigInt,
        keywords -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_memory_documents (id) {
        id -> Text,
        namespace -> Text,
        source -> Text,
        title -> Text,
        created_at -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_messages (id) {
        id -> Text,
        session_id -> Text,
        run_id -> Nullable<Text>,
        created_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_plan_steps (id) {
        id -> Text,
        plan_id -> Text,
        ordinal -> BigInt,
        status -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_plans (id) {
        id -> Text,
        task_id -> Text,
        version -> BigInt,
        status -> Text,
        created_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_providers (id) {
        id -> Text,
        display_name -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_run_events (id) {
        id -> Text,
        run_id -> Text,
        session_id -> Text,
        sequence -> BigInt,
        event_type -> Text,
        created_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_run_steps (id) {
        id -> Text,
        run_id -> Text,
        task_id -> Text,
        sequence -> BigInt,
        started_at -> Text,
        finished_at -> Nullable<Text>,
        data -> Text,
    }
}

diesel::table! {
    agent_runs (id) {
        id -> Text,
        session_id -> Text,
        task_id -> Text,
        started_at -> Text,
        finished_at -> Nullable<Text>,
        data -> Text,
    }
}

diesel::table! {
    agent_session_skill_bindings (session_id, skill_id) {
        session_id -> Text,
        skill_id -> Text,
        updated_at -> Text,
        order_index -> BigInt,
        data -> Text,
    }
}

diesel::table! {
    agent_session_skill_policies (session_id) {
        session_id -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_sessions (id) {
        id -> Text,
        created_at -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_skills (id) {
        id -> Text,
        name -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_subagents (id) {
        id -> Text,
        name -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_tasks (id) {
        id -> Text,
        session_id -> Text,
        updated_at -> Text,
        data -> Text,
    }
}

diesel::table! {
    agent_tool_invocations (id) {
        id -> Text,
        run_id -> Text,
        run_step_id -> Text,
        tool_name -> Text,
        started_at -> Text,
        finished_at -> Text,
        data -> Text,
    }
}
