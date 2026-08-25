//! Delegation trace records for SubAgent collaboration.

use serde::{Deserialize, Serialize};

use crate::subagent_error::{SubAgentError, SubAgentErrorInfo, redact_secret_like};

fn default_id() -> String {
    agent_scope_utils::id::generate_id()
}

/// Stable SubAgent delegation lifecycle event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationEventType {
    TemplateValidated,
    SubAgentRegistered,
    DelegationRequested,
    SubAgentSelected,
    SubAgentStarted,
    SubAgentEventForwarded,
    SubAgentCompleted,
    SubAgentFailed,
    SubAgentTimedOut,
    SubAgentCancelled,
    ScopeDenied,
    BudgetExceeded,
    UnsupportedFeature,
    ResultObservedByParent,
}

impl DelegationEventType {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::SubAgentCompleted
                | Self::SubAgentFailed
                | Self::SubAgentTimedOut
                | Self::SubAgentCancelled
                | Self::ScopeDenied
                | Self::BudgetExceeded
                | Self::UnsupportedFeature
        )
    }
}

/// One ordered event inside a delegation trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegationEvent {
    pub sequence: u64,
    pub event_type: DelegationEventType,
    pub agent_name: String,
    pub delegation_id: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SubAgentErrorInfo>,
}

/// Structured trace for one parent-to-SubAgent delegation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegationTrace {
    #[serde(default = "default_id")]
    pub trace_id: String,
    pub parent_reply_id: String,
    pub delegation_id: String,
    pub parent_agent_name: String,
    pub target_subagent_name: String,
    #[serde(default)]
    pub events: Vec<DelegationEvent>,
    #[serde(default)]
    pub redactions: Vec<String>,
}

impl DelegationTrace {
    pub fn new(
        parent_reply_id: impl Into<String>,
        delegation_id: impl Into<String>,
        parent_agent_name: impl Into<String>,
        target_subagent_name: impl Into<String>,
    ) -> Self {
        Self {
            trace_id: default_id(),
            parent_reply_id: parent_reply_id.into(),
            delegation_id: delegation_id.into(),
            parent_agent_name: parent_agent_name.into(),
            target_subagent_name: target_subagent_name.into(),
            events: Vec::new(),
            redactions: Vec::new(),
        }
    }

    pub fn append(
        &mut self,
        event_type: DelegationEventType,
        agent_name: impl Into<String>,
        summary: impl Into<String>,
    ) {
        self.append_with_error(event_type, agent_name, summary, None);
    }

    pub fn append_error(
        &mut self,
        event_type: DelegationEventType,
        agent_name: impl Into<String>,
        error: &SubAgentError,
    ) {
        self.append_with_error(
            event_type,
            agent_name,
            error.to_string(),
            Some(error.info()),
        );
    }

    pub fn append_with_error(
        &mut self,
        event_type: DelegationEventType,
        agent_name: impl Into<String>,
        summary: impl Into<String>,
        error: Option<SubAgentErrorInfo>,
    ) {
        let sequence = self.events.len() as u64 + 1;
        self.events.push(DelegationEvent {
            sequence,
            event_type,
            agent_name: agent_name.into(),
            delegation_id: self.delegation_id.clone(),
            summary: safe_summary(summary.into()),
            error,
        });
    }

    pub fn validate_terminal(&self) -> Result<(), SubAgentError> {
        let terminal_count = self
            .events
            .iter()
            .filter(|e| e.event_type.is_terminal())
            .count();
        if terminal_count == 1 {
            Ok(())
        } else {
            Err(SubAgentError::InternalError {
                message: format!(
                    "expected exactly one terminal delegation event, got {terminal_count}"
                ),
            })
        }
    }

    pub fn has_event(&self, event_type: DelegationEventType) -> bool {
        self.events
            .iter()
            .any(|event| event.event_type == event_type)
    }
}

pub fn safe_summary(summary: impl Into<String>) -> String {
    redact_secret_like(&summary.into())
}
