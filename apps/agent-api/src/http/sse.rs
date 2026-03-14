use agent_core::RunEventEnvelope;
use axum::response::sse::Event;

pub fn encode_run_event(event: &RunEventEnvelope) -> Event {
    Event::default()
        .event(&event.event_type)
        .json_data(event)
        .expect("run event serialization")
}
