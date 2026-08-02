//! SubAgent typed errors and stable error categories.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::agent_error::AgentError;

/// Stable machine-readable SubAgent error category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentErrorCategory {
    InvalidTemplate,
    DuplicateSubAgent,
    MissingSubAgent,
    DisabledSubAgent,
    AmbiguousSubAgent,
    InvalidDelegation,
    ExecutionFailure,
    Timeout,
    Cancellation,
    PermissionDenied,
    BudgetExceeded,
    UnsupportedFeature,
    InternalError,
}

impl SubAgentErrorCategory {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidTemplate => "invalid_template",
            Self::DuplicateSubAgent => "duplicate_subagent",
            Self::MissingSubAgent => "missing_subagent",
            Self::DisabledSubAgent => "disabled_subagent",
            Self::AmbiguousSubAgent => "ambiguous_subagent",
            Self::InvalidDelegation => "invalid_delegation",
            Self::ExecutionFailure => "execution_failure",
            Self::Timeout => "timeout",
            Self::Cancellation => "cancellation",
            Self::PermissionDenied => "permission_denied",
            Self::BudgetExceeded => "budget_exceeded",
            Self::UnsupportedFeature => "unsupported_feature",
            Self::InternalError => "internal_error",
        }
    }
}

/// Redacted, serializable error info returned in collaboration results and traces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubAgentErrorInfo {
    pub code: String,
    pub category: SubAgentErrorCategory,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl SubAgentErrorInfo {
    pub fn new(category: SubAgentErrorCategory, message: impl Into<String>) -> Self {
        Self {
            code: category.code().to_string(),
            category,
            message: redact_secret_like(&message.into()),
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(redact_secret_like(&source.into()));
        self
    }
}

/// Typed error for SubAgent collaboration operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubAgentError {
    InvalidTemplate { reason: String },
    DuplicateSubAgent { name: String },
    MissingSubAgent { name: String },
    DisabledSubAgent { name: String },
    AmbiguousSubAgent { query: String },
    InvalidDelegation { reason: String },
    ExecutionFailure { agent: String, message: String },
    Timeout { agent: String, timeout_ms: u64 },
    Cancellation { agent: String },
    PermissionDenied { capability: String, reason: String },
    BudgetExceeded { limit: String, value: String },
    UnsupportedFeature { feature: String, reason: String },
    InternalError { message: String },
}

impl SubAgentError {
    pub fn category(&self) -> SubAgentErrorCategory {
        match self {
            Self::InvalidTemplate { .. } => SubAgentErrorCategory::InvalidTemplate,
            Self::DuplicateSubAgent { .. } => SubAgentErrorCategory::DuplicateSubAgent,
            Self::MissingSubAgent { .. } => SubAgentErrorCategory::MissingSubAgent,
            Self::DisabledSubAgent { .. } => SubAgentErrorCategory::DisabledSubAgent,
            Self::AmbiguousSubAgent { .. } => SubAgentErrorCategory::AmbiguousSubAgent,
            Self::InvalidDelegation { .. } => SubAgentErrorCategory::InvalidDelegation,
            Self::ExecutionFailure { .. } => SubAgentErrorCategory::ExecutionFailure,
            Self::Timeout { .. } => SubAgentErrorCategory::Timeout,
            Self::Cancellation { .. } => SubAgentErrorCategory::Cancellation,
            Self::PermissionDenied { .. } => SubAgentErrorCategory::PermissionDenied,
            Self::BudgetExceeded { .. } => SubAgentErrorCategory::BudgetExceeded,
            Self::UnsupportedFeature { .. } => SubAgentErrorCategory::UnsupportedFeature,
            Self::InternalError { .. } => SubAgentErrorCategory::InternalError,
        }
    }

    pub fn info(&self) -> SubAgentErrorInfo {
        SubAgentErrorInfo::new(self.category(), self.to_string())
    }

    pub fn unsupported(feature: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::UnsupportedFeature {
            feature: feature.into(),
            reason: reason.into(),
        }
    }

    pub fn from_agent_error(agent: impl Into<String>, err: AgentError) -> Self {
        let agent = agent.into();
        match err {
            AgentError::TimeoutError { duration, .. } => Self::Timeout {
                agent,
                timeout_ms: duration.as_millis().min(u128::from(u64::MAX)) as u64,
            },
            AgentError::CancellationError { .. } => Self::Cancellation { agent },
            AgentError::PermissionDenied { tool_name, reason } => Self::PermissionDenied {
                capability: tool_name,
                reason,
            },
            AgentError::AlreadyStreaming => Self::ExecutionFailure {
                agent,
                message: "target agent is already processing a reply".to_string(),
            },
            other => Self::ExecutionFailure {
                agent,
                message: other.to_string(),
            },
        }
    }
}

impl fmt::Display for SubAgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTemplate { reason } => write!(f, "Invalid SubAgent template: {reason}"),
            Self::DuplicateSubAgent { name } => write!(f, "Duplicate SubAgent: {name}"),
            Self::MissingSubAgent { name } => write!(f, "SubAgent not found: {name}"),
            Self::DisabledSubAgent { name } => write!(f, "SubAgent disabled: {name}"),
            Self::AmbiguousSubAgent { query } => write!(f, "Ambiguous SubAgent selection: {query}"),
            Self::InvalidDelegation { reason } => write!(f, "Invalid delegation: {reason}"),
            Self::ExecutionFailure { agent, message } => {
                write!(f, "SubAgent '{agent}' failed: {message}")
            }
            Self::Timeout { agent, timeout_ms } => {
                write!(f, "SubAgent '{agent}' timed out after {timeout_ms}ms")
            }
            Self::Cancellation { agent } => write!(f, "SubAgent '{agent}' was cancelled"),
            Self::PermissionDenied { capability, reason } => {
                write!(f, "Permission denied for '{capability}': {reason}")
            }
            Self::BudgetExceeded { limit, value } => {
                write!(f, "Delegation budget exceeded for {limit}: {value}")
            }
            Self::UnsupportedFeature { feature, reason } => {
                write!(f, "Unsupported feature '{feature}': {reason}")
            }
            Self::InternalError { message } => write!(f, "Internal SubAgent error: {message}"),
        }
    }
}

impl std::error::Error for SubAgentError {}

/// Conservative secret-like redaction for default traces and diagnostics.
pub(crate) fn redact_secret_like(input: &str) -> String {
    let mut out = Vec::new();
    for token in input.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        if lower.contains("api_key")
            || lower.contains("apikey")
            || lower.contains("access_token")
            || lower.contains("token=")
            || lower.contains("secret")
            || lower.contains("password")
            || token.starts_with("sk-")
        {
            out.push("[REDACTED]".to_string());
        } else {
            out.push(token.to_string());
        }
    }
    out.join(" ")
}
