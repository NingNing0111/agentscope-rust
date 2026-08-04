//! Delegation request/result types and in-process SubAgent orchestration.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use tokio_util::sync::CancellationToken;

use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};

use crate::agent_trait::Agent;
use crate::context_policy::SharedContext;
use crate::delegation_trace::{DelegationEventType, DelegationTrace};
use crate::subagent::SubAgentRegistry;
use crate::subagent_error::{SubAgentError, SubAgentErrorInfo};

fn default_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

/// How delegation results are returned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationReplyMode {
    #[default]
    FinalOnly,
    StreamEvents,
    ObserveOnly,
}

/// Delegation budget and limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationBudget {
    pub max_depth: u32,
    pub max_calls: u32,
    pub timeout_ms: u64,
    pub max_context_messages: usize,
    pub allow_concurrent: bool,
}

impl Default for DelegationBudget {
    fn default() -> Self {
        Self {
            max_depth: 1,
            max_calls: 1,
            timeout_ms: 30_000,
            max_context_messages: 32,
            allow_concurrent: false,
        }
    }
}

impl DelegationBudget {
    pub fn validate(&self) -> Result<(), SubAgentError> {
        if self.max_depth == 0 {
            return Err(SubAgentError::BudgetExceeded {
                limit: "max_depth".to_string(),
                value: "0".to_string(),
            });
        }
        if self.max_calls == 0 {
            return Err(SubAgentError::BudgetExceeded {
                limit: "max_calls".to_string(),
                value: "0".to_string(),
            });
        }
        if self.timeout_ms == 0 {
            return Err(SubAgentError::BudgetExceeded {
                limit: "timeout_ms".to_string(),
                value: "0".to_string(),
            });
        }
        Ok(())
    }
}

/// A request from a parent agent to one SubAgent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRequest {
    #[serde(default = "default_id")]
    pub delegation_id: String,
    pub parent_agent_name: String,
    pub target_subagent_name: String,
    pub task: String,
    pub context: SharedContext,
    #[serde(default)]
    pub budget: DelegationBudget,
    #[serde(default)]
    pub reply_mode: DelegationReplyMode,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl DelegationRequest {
    pub fn new(
        parent_agent_name: impl Into<String>,
        target_subagent_name: impl Into<String>,
        task: impl Into<String>,
    ) -> Self {
        Self {
            delegation_id: default_id(),
            parent_agent_name: parent_agent_name.into(),
            target_subagent_name: target_subagent_name.into(),
            task: task.into(),
            context: SharedContext::empty(),
            budget: DelegationBudget::default(),
            reply_mode: DelegationReplyMode::FinalOnly,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    pub fn validate(&self) -> Result<(), SubAgentError> {
        if self.parent_agent_name.trim().is_empty() {
            return Err(SubAgentError::InvalidDelegation {
                reason: "parent_agent_name must not be empty".to_string(),
            });
        }
        if self.target_subagent_name.trim().is_empty() {
            return Err(SubAgentError::InvalidDelegation {
                reason: "target_subagent_name must not be empty".to_string(),
            });
        }
        if self.task.trim().is_empty() {
            return Err(SubAgentError::InvalidDelegation {
                reason: "task must not be empty".to_string(),
            });
        }
        if self.context.messages.len() > self.budget.max_context_messages {
            return Err(SubAgentError::BudgetExceeded {
                limit: "max_context_messages".to_string(),
                value: self.context.messages.len().to_string(),
            });
        }
        self.budget.validate()
    }
}

/// Terminal status of a SubAgent collaboration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationStatus {
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
    PermissionDenied,
    UnsupportedFeature,
}

/// Side effects attributed to a SubAgent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideEffectRecord {
    pub effect_id: String,
    pub subagent_name: String,
    pub effect_type: SideEffectType,
    pub scope: String,
    pub summary: String,
}

/// Types of side effects that can be attributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectType {
    MemoryWrite,
    SessionUpdate,
    WorkspaceWrite,
    ToolInvocation,
    ModelCall,
}

/// Participant in a multi-agent conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub name: String,
    pub role: String,
}

/// Ordered conversation record preserving parent/SubAgent speaker identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAgentConversation {
    pub conversation_id: String,
    #[serde(default)]
    pub participants: Vec<Participant>,
    #[serde(default)]
    pub messages: Vec<Msg>,
    #[serde(default)]
    pub delegations: Vec<String>,
}

impl MultiAgentConversation {
    pub fn new(conversation_id: impl Into<String>) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            participants: Vec::new(),
            messages: Vec::new(),
            delegations: Vec::new(),
        }
    }

    pub fn add_participant(&mut self, name: impl Into<String>, role: impl Into<String>) {
        let name = name.into();
        if !self.participants.iter().any(|p| p.name == name) {
            self.participants.push(Participant {
                name,
                role: role.into(),
            });
        }
    }

    pub fn push_message(&mut self, msg: Msg) {
        self.messages.push(msg);
    }
}

/// Attributable result returned from a SubAgent to the parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationResult {
    pub delegation_id: String,
    pub subagent_name: String,
    pub status: CollaborationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Msg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SubAgentErrorInfo>,
    pub trace_id: String,
    pub trace: DelegationTrace,
    #[serde(default)]
    pub side_effects: Vec<SideEffectRecord>,
}

impl CollaborationResult {
    pub fn succeeded(
        request: &DelegationRequest,
        mut message: Msg,
        mut trace: DelegationTrace,
    ) -> Self {
        message.name = request.target_subagent_name.clone();
        let trace_id = trace.trace_id.clone();
        trace.append(
            DelegationEventType::SubAgentCompleted,
            &request.target_subagent_name,
            "SubAgent completed successfully",
        );
        Self {
            delegation_id: request.delegation_id.clone(),
            subagent_name: request.target_subagent_name.clone(),
            status: CollaborationStatus::Succeeded,
            message: Some(message),
            error: None,
            trace_id,
            trace,
            side_effects: Vec::new(),
        }
    }

    pub fn failed(
        request: &DelegationRequest,
        error: SubAgentError,
        mut trace: DelegationTrace,
    ) -> Self {
        let status = match error {
            SubAgentError::Timeout { .. } => CollaborationStatus::TimedOut,
            SubAgentError::Cancellation { .. } => CollaborationStatus::Cancelled,
            SubAgentError::PermissionDenied { .. } => CollaborationStatus::PermissionDenied,
            SubAgentError::UnsupportedFeature { .. } => CollaborationStatus::UnsupportedFeature,
            _ => CollaborationStatus::Failed,
        };
        let event = match status {
            CollaborationStatus::TimedOut => DelegationEventType::SubAgentTimedOut,
            CollaborationStatus::Cancelled => DelegationEventType::SubAgentCancelled,
            CollaborationStatus::PermissionDenied => DelegationEventType::ScopeDenied,
            CollaborationStatus::UnsupportedFeature => DelegationEventType::UnsupportedFeature,
            CollaborationStatus::Failed => DelegationEventType::SubAgentFailed,
            CollaborationStatus::Succeeded => DelegationEventType::SubAgentCompleted,
        };
        trace.append_error(event, &request.target_subagent_name, &error);
        let trace_id = trace.trace_id.clone();
        Self {
            delegation_id: request.delegation_id.clone(),
            subagent_name: request.target_subagent_name.clone(),
            status,
            message: None,
            error: Some(error.info()),
            trace_id,
            trace,
            side_effects: Vec::new(),
        }
    }
}

/// Execute one final-only in-process SubAgent delegation.
pub async fn delegate_once(
    registry: &SubAgentRegistry,
    request: DelegationRequest,
) -> Result<CollaborationResult, SubAgentError> {
    delegate_once_with_cancel(registry, request, None).await
}

/// Execute one delegation with optional parent cancellation propagation.
pub async fn delegate_once_with_cancel(
    registry: &SubAgentRegistry,
    request: DelegationRequest,
    cancellation: Option<CancellationToken>,
) -> Result<CollaborationResult, SubAgentError> {
    request.validate()?;
    if request.reply_mode != DelegationReplyMode::FinalOnly {
        return Err(SubAgentError::unsupported(
            "delegate_stream",
            "use delegate_stream for streaming delegation",
        ));
    }

    let mut trace = DelegationTrace::new(
        request.delegation_id.clone(),
        request.delegation_id.clone(),
        request.parent_agent_name.clone(),
        request.target_subagent_name.clone(),
    );
    trace.append(
        DelegationEventType::DelegationRequested,
        &request.parent_agent_name,
        format!("Delegating task to {}", request.target_subagent_name),
    );

    let target = registry.get(&request.target_subagent_name)?;
    let agent = target.agent.clone();
    trace.append(
        DelegationEventType::SubAgentSelected,
        &request.parent_agent_name,
        format!("Selected SubAgent {}", request.target_subagent_name),
    );

    let mut input = request.context.messages.clone();
    input.push(task_msg(&request.parent_agent_name, &request.task));

    trace.append(
        DelegationEventType::SubAgentStarted,
        &request.target_subagent_name,
        "SubAgent started processing delegated task",
    );

    let reply_future = timeout(
        Duration::from_millis(request.budget.timeout_ms),
        agent.reply(Some(input)),
    );
    let reply = if let Some(token) = cancellation {
        tokio::select! {
            _ = token.cancelled() => {
                let error = SubAgentError::Cancellation { agent: request.target_subagent_name.clone() };
                return Ok(CollaborationResult::failed(&request, error, trace));
            }
            result = reply_future => result
        }
    } else {
        reply_future.await
    };

    match reply {
        Ok(Ok(msg)) => Ok(CollaborationResult::succeeded(&request, msg, trace)),
        Ok(Err(err)) => {
            let error = SubAgentError::from_agent_error(request.target_subagent_name.clone(), err);
            Ok(CollaborationResult::failed(&request, error, trace))
        }
        Err(_) => {
            let error = SubAgentError::Timeout {
                agent: request.target_subagent_name.clone(),
                timeout_ms: request.budget.timeout_ms,
            };
            Ok(CollaborationResult::failed(&request, error, trace))
        }
    }
}

/// Execute multiple delegation requests. Sequential execution is the deterministic default.
pub async fn delegate_many(
    registry: &SubAgentRegistry,
    requests: Vec<DelegationRequest>,
) -> Result<Vec<CollaborationResult>, SubAgentError> {
    let allow_concurrent = requests.iter().any(|r| r.budget.allow_concurrent);
    if !allow_concurrent {
        let mut results = Vec::with_capacity(requests.len());
        for request in requests {
            results.push(delegate_once(registry, request).await?);
        }
        return Ok(results);
    }

    // `allow_concurrent` was requested: run the delegations in parallel.
    // `join_all` collects the results in input order (previously this branch
    // ran the same sequential loop as the non-concurrent path, silently
    // ignoring the flag — audit A2). The first error is propagated, matching
    // the sequential path's fail-fast semantics.
    let results = futures::future::join_all(
        requests
            .into_iter()
            .map(|request| delegate_once(registry, request)),
    )
    .await;
    let mut out = Vec::with_capacity(results.len());
    for result in results {
        out.push(result?);
    }
    Ok(out)
}

/// Stream SubAgent events with trace correlation and a terminal result.
pub async fn delegate_stream(
    registry: &SubAgentRegistry,
    mut request: DelegationRequest,
) -> Result<(mpsc::Receiver<DelegationEventType>, CollaborationResult), SubAgentError> {
    let (tx, rx) = mpsc::channel(16);
    request.reply_mode = DelegationReplyMode::FinalOnly;
    let mut result = delegate_once(registry, request).await?;
    result.trace.append(
        DelegationEventType::SubAgentEventForwarded,
        result.subagent_name.clone(),
        "Forwarded correlated SubAgent event",
    );
    let _ = tx.send(DelegationEventType::SubAgentEventForwarded).await;
    let terminal = result
        .trace
        .events
        .iter()
        .find(|event| event.event_type.is_terminal())
        .map(|event| event.event_type);
    if let Some(event_type) = terminal {
        let _ = tx.send(event_type).await;
    }
    drop(tx);
    Ok((rx, result))
}

/// Observe a successful SubAgent result in the parent agent context.
pub async fn observe_result_by_parent(
    parent: &dyn Agent,
    result: &mut CollaborationResult,
) -> Result<(), SubAgentError> {
    let msg = result
        .message
        .clone()
        .ok_or_else(|| SubAgentError::InvalidDelegation {
            reason: "only successful collaboration results can be observed by parent".to_string(),
        })?;
    parent
        .observe(Some(vec![msg]))
        .await
        .map_err(|err| SubAgentError::from_agent_error(result.subagent_name.clone(), err))?;
    result.trace.append(
        DelegationEventType::ResultObservedByParent,
        parent.name(),
        "Parent observed successful SubAgent result",
    );
    Ok(())
}

pub fn unsupported_remote_worker() -> SubAgentError {
    SubAgentError::unsupported(
        "remote_worker",
        "remote SubAgent workers are deferred to distributed runtime support",
    )
}

pub fn unsupported_durable_queue() -> SubAgentError {
    SubAgentError::unsupported(
        "durable_queue",
        "durable external queues are outside in-process SubAgent collaboration",
    )
}

pub fn unsupported_cross_host_migration() -> SubAgentError {
    SubAgentError::unsupported(
        "cross_host_migration",
        "cross-host SubAgent migration is deferred to distributed runtime support",
    )
}

pub fn unsupported_app_service_dispatch() -> SubAgentError {
    SubAgentError::unsupported(
        "app_service_dispatch",
        "full Python app-service dispatch compatibility is deferred",
    )
}

fn task_msg(parent_agent_name: &str, task: &str) -> Msg {
    Msg::new(
        parent_agent_name.to_string(),
        vec![ContentBlock::Text(TextBlock::new(task.to_string()))],
        Role::User,
    )
    .expect("delegated task user text message is valid")
}
