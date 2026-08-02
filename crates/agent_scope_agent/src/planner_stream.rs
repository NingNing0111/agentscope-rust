//! Streaming helpers for Planner events.

use std::collections::HashMap;
use std::pin::Pin;

use agent_scope_event::{AgentEvent, CustomEvent, EventBase};
use futures::{Stream, stream};

use crate::planning_trace::{PlanningEvent, PlanningTrace};

/// Convert a planning trace into a stream of public AgentEvent values.
pub fn trace_to_stream(trace: PlanningTrace) -> Pin<Box<dyn Stream<Item = AgentEvent> + Send>> {
    let events: Vec<AgentEvent> = trace
        .events
        .into_iter()
        .map(planning_event_to_agent_event)
        .collect();
    Box::pin(stream::iter(events))
}

/// Convert a single planning event into a public custom AgentEvent.
pub fn planning_event_to_agent_event(event: PlanningEvent) -> AgentEvent {
    let mut value = HashMap::new();
    value.insert("sequence".into(), serde_json::json!(event.sequence));
    value.insert("event_type".into(), serde_json::json!(event.event_type));
    value.insert("task_id".into(), serde_json::json!(event.task_id));
    if let Some(plan_id) = event.plan_id {
        value.insert("plan_id".into(), serde_json::json!(plan_id));
    }
    if let Some(step_id) = event.step_id {
        value.insert("step_id".into(), serde_json::json!(step_id));
    }
    if let Some(summary) = event.summary {
        value.insert("summary".into(), serde_json::json!(summary));
    }
    if let Some(error) = event.error {
        value.insert(
            "error".into(),
            serde_json::to_value(error).unwrap_or(serde_json::Value::Null),
        );
    }
    AgentEvent::Custom(CustomEvent {
        base: EventBase::new(),
        name: "planner.lifecycle".into(),
        value,
    })
}
