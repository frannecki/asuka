use std::convert::Infallible;

use agent_core::RunRecord;
use async_stream::stream;
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    state::ApiState,
};

use super::sse::encode_run_event;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/runs/:run_id", get(get_run))
        .route("/runs/:run_id/cancel", post(cancel_run))
        .route("/runs/:run_id/events", get(stream_run_events))
}

async fn get_run(State(state): State<ApiState>, Path(run_id): Path<Uuid>) -> ApiResult<RunRecord> {
    Ok(Json(state.core.get_run(run_id).await?))
}

async fn cancel_run(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<RunRecord> {
    Ok(Json(state.core.cancel_run(run_id).await?))
}

async fn stream_run_events(
    State(state): State<ApiState>,
    Path(run_id): Path<Uuid>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let run = state.core.get_run(run_id).await?;
    let mut rx = state.core.subscribe_events();
    let ready_event = state.core.stream_ready_event(run_id, run.session_id);

    let event_stream = stream! {
        yield Ok::<Event, Infallible>(encode_run_event(&ready_event));

        loop {
            match rx.recv().await {
                Ok(event) if event.run_id == run_id => {
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
