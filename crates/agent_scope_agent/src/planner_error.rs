//! Typed errors for Planner + ReActAgent orchestration.

#![allow(clippy::result_large_err)]

use std::fmt;
use std::time::Duration;

use crate::agent_error::AgentError;

/// Stable machine-readable planner error category.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannerErrorCategory {
    InvalidGoal,
    PlanGenerationFailed,
    MalformedPlan,
    NonActionablePlan,
    StepExecutionFailed,
    ReplanningFailed,
    StepLimitExceeded,
    ReplanLimitExceeded,
    Timeout,
    Cancelled,
    PermissionDenied,
    UnsupportedCapability,
    TraceSerializationFailed,
    InternalError,
}

impl fmt::Display for PlannerErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::InvalidGoal => "InvalidGoal",
            Self::PlanGenerationFailed => "PlanGenerationFailed",
            Self::MalformedPlan => "MalformedPlan",
            Self::NonActionablePlan => "NonActionablePlan",
            Self::StepExecutionFailed => "StepExecutionFailed",
            Self::ReplanningFailed => "ReplanningFailed",
            Self::StepLimitExceeded => "StepLimitExceeded",
            Self::ReplanLimitExceeded => "ReplanLimitExceeded",
            Self::Timeout => "Timeout",
            Self::Cancelled => "Cancelled",
            Self::PermissionDenied => "PermissionDenied",
            Self::UnsupportedCapability => "UnsupportedCapability",
            Self::TraceSerializationFailed => "TraceSerializationFailed",
            Self::InternalError => "InternalError",
        };
        f.write_str(s)
    }
}

/// Typed error details for planner operations.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlannerError {
    pub category: PlannerErrorCategory,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_category: Option<String>,
    pub retryable: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl PlannerError {
    /// Create a planner error.
    pub fn new(category: PlannerErrorCategory, message: impl Into<String>) -> Self {
        let retryable = matches!(
            category,
            PlannerErrorCategory::StepExecutionFailed | PlannerErrorCategory::ReplanningFailed
        );
        Self {
            category,
            message: message.into(),
            task_id: None,
            plan_id: None,
            step_id: None,
            source_category: None,
            retryable,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Attach task/plan/step context.
    pub fn with_context(
        mut self,
        task_id: Option<String>,
        plan_id: Option<String>,
        step_id: Option<String>,
    ) -> Self {
        self.task_id = task_id;
        self.plan_id = plan_id;
        self.step_id = step_id;
        self
    }

    /// Mark this error retryable or non-retryable.
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// Create an unsupported capability error.
    pub fn unsupported(capability: impl Into<String>) -> Self {
        let capability = capability.into();
        Self::new(
            PlannerErrorCategory::UnsupportedCapability,
            format!("Unsupported planner capability: {capability}"),
        )
        .retryable(false)
    }
}

impl fmt::Display for PlannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.category, self.message)
    }
}

impl std::error::Error for PlannerError {}

impl From<PlannerError> for AgentError {
    fn from(error: PlannerError) -> Self {
        match error.category {
            PlannerErrorCategory::InvalidGoal
            | PlannerErrorCategory::MalformedPlan
            | PlannerErrorCategory::NonActionablePlan
            | PlannerErrorCategory::TraceSerializationFailed => AgentError::ValidationError {
                message: error.to_string(),
            },
            PlannerErrorCategory::Timeout => AgentError::TimeoutError {
                operation: "planner".into(),
                duration: Duration::from_secs(0),
            },
            PlannerErrorCategory::Cancelled => AgentError::CancellationError {
                reply_id: error.task_id.unwrap_or_else(|| "planner".into()),
            },
            PlannerErrorCategory::PermissionDenied => AgentError::PermissionDenied {
                tool_name: error.step_id.unwrap_or_else(|| "planner-step".into()),
                reason: error.message,
            },
            PlannerErrorCategory::StepLimitExceeded => {
                AgentError::MaxItersExceeded { max_iters: 0 }
            }
            PlannerErrorCategory::UnsupportedCapability
            | PlannerErrorCategory::PlanGenerationFailed
            | PlannerErrorCategory::StepExecutionFailed
            | PlannerErrorCategory::ReplanningFailed
            | PlannerErrorCategory::ReplanLimitExceeded
            | PlannerErrorCategory::InternalError => AgentError::ValidationError {
                message: error.to_string(),
            },
        }
    }
}

impl From<AgentError> for PlannerError {
    fn from(error: AgentError) -> Self {
        match error {
            AgentError::ValidationError { message } | AgentError::InvalidConfig { message, .. } => {
                Self::new(PlannerErrorCategory::MalformedPlan, message)
            }
            AgentError::ModelError { source } => Self::new(
                PlannerErrorCategory::PlanGenerationFailed,
                source.to_string(),
            ),
            AgentError::ToolError { source } => Self::new(
                PlannerErrorCategory::StepExecutionFailed,
                source.to_string(),
            ),
            AgentError::TimeoutError { operation, .. } => {
                Self::new(PlannerErrorCategory::Timeout, operation).retryable(false)
            }
            AgentError::CancellationError { reply_id } => {
                Self::new(PlannerErrorCategory::Cancelled, reply_id).retryable(false)
            }
            AgentError::PermissionDenied { tool_name, reason } => Self::new(
                PlannerErrorCategory::PermissionDenied,
                format!("{tool_name}: {reason}"),
            )
            .retryable(false),
            AgentError::ContextCompressionFailed { reason } => {
                Self::new(PlannerErrorCategory::StepExecutionFailed, reason)
            }
            AgentError::NoContentToReply => {
                Self::new(PlannerErrorCategory::InvalidGoal, "no content to plan from")
            }
            AgentError::MaxItersExceeded { max_iters } => Self::new(
                PlannerErrorCategory::StepLimitExceeded,
                format!("max iterations exceeded: {max_iters}"),
            )
            .retryable(false),
            AgentError::AlreadyStreaming => Self::new(
                PlannerErrorCategory::InternalError,
                "agent is already streaming",
            ),
        }
    }
}
