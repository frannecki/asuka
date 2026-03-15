use std::convert::Infallible;

use agent_core::{RunEventHistory, RunRecord, RunStepRecord, ToolInvocationRecord};
use async_stream::stream;
use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    state::ApiState,
};

use super::sse::encode_run_event;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/runs/:run_id", get(get_run))
        .route("/runs/:run_id/steps", get(list_run_steps))
        .route("/runs/:run_id/tool-invocations", get(list_tool_invocations))
        .route("/runs/:run_id/cancel", post(cancel_run))
        .route("/runs/:run_id/events/history", get(get_run_event_history))
        .route("/runs/:run_id/events", get(stream_run_events))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunEventsQuery {
    #[serde(alias = "after_sequence")]
    after_sequence: Option<u64>,
}

async fn get_run(State(state): State<ApiState>, Path(run_id): Path<Uuid>) -> ApiResult<RunRecord> {
    Ok(Json(state.core.get_run(run_id).await?))
}

async fn list_run_steps(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Vec<RunStepRecord>> {
    Ok(Json(state.core.list_run_steps(run_id).await?))
}

async fn list_tool_invocations(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Vec<ToolInvocationRecord>> {
    Ok(Json(state.core.list_tool_invocations(run_id).await?))
}

async fn cancel_run(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<RunRecord> {
    Ok(Json(state.core.cancel_run(run_id).await?))
}

async fn get_run_event_history(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
    Query(query): Query<RunEventsQuery>,
) -> ApiResult<RunEventHistory> {
    Ok(Json(
        state
            .core
            .list_run_events(run_id, query.after_sequence)
            .await?,
    ))
}

async fn stream_run_events(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
    Query(query): Query<RunEventsQuery>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let run = state.core.get_run(run_id).await?;
    let mut rx = state.core.subscribe_events();
    let history = state
        .core
        .list_run_events(run_id, query.after_sequence)
        .await?;
    let mut last_sequence = history.last_sequence;
    let ready_event = state.core.stream_ready_event(run_id, run.session_id);

    let event_stream = stream! {
        yield Ok::<Event, Infallible>(encode_run_event(&ready_event));
        for event in history.events {
            last_sequence = event.sequence;
            yield Ok::<Event, Infallible>(encode_run_event(&event));
        }

        loop {
            match rx.recv().await {
                Ok(event) if event.run_id == run_id && event.sequence > last_sequence => {
                    last_sequence = event.sequence;
                    yield Ok::<Event, Infallible>(encode_run_event(&event));
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(10))
            .text("keep-alive"),
    ))
}
