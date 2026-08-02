#[path = "support/subagent_test_agent.rs"]
mod subagent_test_agent;

use std::sync::Arc;
use std::time::Duration;

use agent_scope_agent::*;
use subagent_test_agent::ScriptedTestAgent;

#[tokio::test]
async fn execution_failure_maps_to_failed_result() {
    let failing = Arc::new(ScriptedTestAgent::new("researcher", "never").failing("boom"));
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(SubAgent::new("researcher", "research", failing).unwrap())
        .unwrap();
    let result = delegate_once(
        &registry,
        DelegationRequest::new("planner", "researcher", "task"),
    )
    .await
    .unwrap();
    assert_eq!(result.status, CollaborationStatus::Failed);
    assert_eq!(
        result.error.unwrap().category,
        SubAgentErrorCategory::ExecutionFailure
    );
}

#[tokio::test]
async fn timeout_maps_to_timed_out_result() {
    let slow =
        Arc::new(ScriptedTestAgent::new("researcher", "late").delayed(Duration::from_millis(50)));
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(SubAgent::new("researcher", "research", slow).unwrap())
        .unwrap();
    let mut request = DelegationRequest::new("planner", "researcher", "task");
    request.budget.timeout_ms = 1;
    let result = delegate_once(&registry, request).await.unwrap();
    assert_eq!(result.status, CollaborationStatus::TimedOut);
    assert!(
        result
            .trace
            .has_event(DelegationEventType::SubAgentTimedOut)
    );
}

#[tokio::test]
async fn cancellation_maps_to_cancelled_result() {
    let slow =
        Arc::new(ScriptedTestAgent::new("researcher", "late").delayed(Duration::from_millis(100)));
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(SubAgent::new("researcher", "research", slow).unwrap())
        .unwrap();
    let token = tokio_util::sync::CancellationToken::new();
    token.cancel();
    let result = delegate_once_with_cancel(
        &registry,
        DelegationRequest::new("planner", "researcher", "task"),
        Some(token),
    )
    .await
    .unwrap();
    assert_eq!(result.status, CollaborationStatus::Cancelled);
}

#[test]
fn unsupported_patterns_are_typed() {
    assert_eq!(
        unsupported_remote_worker().category(),
        SubAgentErrorCategory::UnsupportedFeature
    );
    assert_eq!(
        unsupported_durable_queue().category(),
        SubAgentErrorCategory::UnsupportedFeature
    );
    assert_eq!(
        unsupported_cross_host_migration().category(),
        SubAgentErrorCategory::UnsupportedFeature
    );
    assert_eq!(
        unsupported_app_service_dispatch().category(),
        SubAgentErrorCategory::UnsupportedFeature
    );
}
