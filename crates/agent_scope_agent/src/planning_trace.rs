//! Planning trace model and redaction helpers.

#![allow(clippy::result_large_err)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::plan::{PlannerOutcome, ToolActivityRecord};
use crate::planner_error::{PlannerError, PlannerErrorCategory};

/// Planner lifecycle event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningEventType {
    PlanningStarted,
    PlanningCompleted,
    PlanValidationFailed,
    StepStarted,
    StepCompleted,
    StepFailed,
    StepSkipped,
    StepCancelled,
    StepUnsupported,
    ReplanningStarted,
    ReplanningCompleted,
    TaskCompleted,
    TaskPartiallyCompleted,
    TaskFailed,
    TaskCancelled,
    TaskUnsupported,
}

impl PlanningEventType {
    /// Returns true for terminal task events.
    pub fn is_task_terminal(self) -> bool {
        matches!(
            self,
            Self::TaskCompleted
                | Self::TaskPartiallyCompleted
                | Self::TaskFailed
                | Self::TaskCancelled
                | Self::TaskUnsupported
        )
    }

    /// Returns true for terminal step events.
    pub fn is_step_terminal(self) -> bool {
        matches!(
            self,
            Self::StepCompleted
                | Self::StepFailed
                | Self::StepSkipped
                | Self::StepCancelled
                | Self::StepUnsupported
        )
    }
}

/// Structured lifecycle event emitted by Planner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningEvent {
    pub sequence: u64,
    pub event_type: PlanningEventType,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_event_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<PlannerError>,
    pub timestamp: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl PlanningEvent {
    /// Create a new event with sequence assigned by `PlanningTrace::push`.
    pub fn new(event_type: PlanningEventType, task_id: impl Into<String>) -> Self {
        Self {
            sequence: 0,
            event_type,
            task_id: task_id.into(),
            plan_id: None,
            step_id: None,
            agent_event_ref: None,
            summary: None,
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Attach plan ID.
    pub fn with_plan(mut self, plan_id: impl Into<String>) -> Self {
        self.plan_id = Some(plan_id.into());
        self
    }

    /// Attach step ID.
    pub fn with_step(mut self, step_id: impl Into<String>) -> Self {
        self.step_id = Some(step_id.into());
        self
    }

    /// Attach redacted summary.
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(safe_summary(&summary.into()));
        self
    }

    /// Attach error details.
    pub fn with_error(mut self, error: PlannerError) -> Self {
        self.error = Some(error);
        self
    }
}

/// Ordered planner trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningTrace {
    pub trace_id: String,
    pub task_id: String,
    pub events: Vec<PlanningEvent>,
    #[serde(default)]
    pub normalized_fields: Vec<String>,
    pub redaction_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_outcome: Option<PlannerOutcome>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl PlanningTrace {
    /// Create an empty trace for a planned task.
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().as_simple().to_string(),
            task_id: task_id.into(),
            events: Vec::new(),
            normalized_fields: Vec::new(),
            redaction_policy: "default-redact-secrets".into(),
            final_outcome: None,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Append an event and assign the next monotonic sequence number.
    pub fn push(&mut self, mut event: PlanningEvent) -> Result<u64, PlannerError> {
        if event.task_id != self.task_id {
            return Err(PlannerError::new(
                PlannerErrorCategory::TraceSerializationFailed,
                "event task_id must match trace task_id",
            ));
        }
        let next = self.events.len() as u64 + 1;
        event.sequence = next;
        self.events.push(event);
        Ok(next)
    }

    /// Append a simple event with optional plan/step IDs.
    pub fn push_event(
        &mut self,
        event_type: PlanningEventType,
        plan_id: Option<String>,
        step_id: Option<String>,
        summary: Option<String>,
    ) -> Result<u64, PlannerError> {
        let mut event = PlanningEvent::new(event_type, self.task_id.clone());
        event.plan_id = plan_id;
        event.step_id = step_id;
        event.summary = summary.map(|s| safe_summary(&s));
        self.push(event)
    }

    /// Set final outcome and emit corresponding terminal event.
    pub fn finish(&mut self, outcome: PlannerOutcome) -> Result<(), PlannerError> {
        let event_type = match &outcome {
            PlannerOutcome::Completed { .. } => PlanningEventType::TaskCompleted,
            PlannerOutcome::PartiallyCompleted { .. } => PlanningEventType::TaskPartiallyCompleted,
            PlannerOutcome::Cancelled { .. } => PlanningEventType::TaskCancelled,
            PlannerOutcome::Failed { .. } => PlanningEventType::TaskFailed,
            PlannerOutcome::Unsupported { .. } => PlanningEventType::TaskUnsupported,
        };
        self.final_outcome = Some(outcome);
        self.push_event(event_type, None, None, None)?;
        Ok(())
    }

    /// Validate sequence ordering and terminal state.
    pub fn validate(&self) -> Result<(), PlannerError> {
        for (idx, event) in self.events.iter().enumerate() {
            let expected = idx as u64 + 1;
            if event.sequence != expected {
                return Err(PlannerError::new(
                    PlannerErrorCategory::TraceSerializationFailed,
                    "planning event sequence must be monotonic",
                ));
            }
            if event.task_id != self.task_id {
                return Err(PlannerError::new(
                    PlannerErrorCategory::TraceSerializationFailed,
                    "event task_id must match trace task_id",
                ));
            }
        }
        if self.final_outcome.is_some()
            && !self
                .events
                .iter()
                .any(|event| event.event_type.is_task_terminal())
        {
            return Err(PlannerError::new(
                PlannerErrorCategory::TraceSerializationFailed,
                "terminal trace must include a task terminal event",
            ));
        }
        Ok(())
    }

    /// Export a compact JSON trace for compatibility fixtures.
    pub fn to_compat_json(&self) -> Result<serde_json::Value, PlannerError> {
        serde_json::to_value(self).map_err(|e| {
            PlannerError::new(
                PlannerErrorCategory::TraceSerializationFailed,
                format!("failed to serialize planning trace: {e}"),
            )
        })
    }
}

/// Redact obvious secret-like values in a human-readable summary.
pub fn safe_summary(input: &str) -> String {
    let mut out = input.to_string();
    for marker in ["api_key", "access_token", "token", "password", "secret"] {
        out = redact_marker(&out, marker);
    }
    out
}

fn redact_marker(input: &str, marker: &str) -> String {
    let lower = input.to_ascii_lowercase();
    if !lower.contains(marker) {
        return input.to_string();
    }
    let mut result = String::with_capacity(input.len());
    for part in input.split_whitespace() {
        if part.to_ascii_lowercase().contains(marker) {
            result.push_str(marker);
            result.push_str("=[REDACTED] ");
        } else {
            result.push_str(part);
            result.push(' ');
        }
    }
    result.trim_end().to_string()
}

/// Redact secret-like keys in a JSON object.
pub fn redact_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (key, value) in map {
                let lower = key.to_ascii_lowercase();
                if ["api_key", "access_token", "token", "password", "secret"]
                    .iter()
                    .any(|marker| lower.contains(marker))
                {
                    redacted.insert(key.clone(), serde_json::Value::String("[REDACTED]".into()));
                } else {
                    redacted.insert(key.clone(), redact_json(value));
                }
            }
            serde_json::Value::Object(redacted)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(redact_json).collect())
        }
        _ => value.clone(),
    }
}

/// Convert tool activity into a redacted metadata map.
pub fn tool_activity_metadata(
    activity: &[ToolActivityRecord],
) -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();
    metadata.insert(
        "tool_activity".into(),
        serde_json::to_value(activity).unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
    );
    metadata
}
