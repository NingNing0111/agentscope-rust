#[path = "support/subagent_test_agent.rs"]
mod subagent_test_agent;

use agent_scope_agent::*;
use subagent_test_agent::{ScriptedTestAgent, scripted_agent};

fn subagent(name: &str, response: &str) -> SubAgent {
    SubAgent::new(
        name,
        format!("{name} responsibility"),
        scripted_agent(name, response),
    )
    .unwrap()
}

#[test]
fn trace_sequence_terminal_and_redaction() {
    let mut trace = DelegationTrace::new("reply", "delegation", "planner", "researcher");
    trace.append(
        DelegationEventType::DelegationRequested,
        "planner",
        "token=abc123 collect facts",
    );
    trace.append(
        DelegationEventType::SubAgentSelected,
        "planner",
        "selected researcher",
    );
    trace.append(
        DelegationEventType::SubAgentStarted,
        "researcher",
        "started",
    );
    trace.append(
        DelegationEventType::SubAgentCompleted,
        "researcher",
        "secret=my-secret done",
    );
    assert_eq!(
        trace.events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert!(trace.validate_terminal().is_ok());
    assert!(!trace.events[0].summary.contains("abc123"));
    assert!(!trace.events[3].summary.contains("my-secret"));
}

#[tokio::test]
async fn trace_order_for_successful_delegation() {
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(subagent("researcher", "facts"))
        .unwrap();
    let mut result = delegate_once(
        &registry,
        DelegationRequest::new("planner", "researcher", "task"),
    )
    .await
    .unwrap();
    let parent = ScriptedTestAgent::new("planner", "final");
    observe_result_by_parent(&parent, &mut result)
        .await
        .unwrap();
    let types = result
        .trace
        .events
        .iter()
        .map(|e| e.event_type)
        .collect::<Vec<_>>();
    assert_eq!(
        types,
        vec![
            DelegationEventType::DelegationRequested,
            DelegationEventType::SubAgentSelected,
            DelegationEventType::SubAgentStarted,
            DelegationEventType::SubAgentCompleted,
            DelegationEventType::ResultObservedByParent
        ]
    );
}

#[tokio::test]
async fn stream_delegation_forwards_correlated_event() {
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(subagent("researcher", "facts"))
        .unwrap();
    let (_rx, result) = delegate_stream(
        &registry,
        DelegationRequest::new("planner", "researcher", "task"),
    )
    .await
    .unwrap();
    assert!(
        result
            .trace
            .has_event(DelegationEventType::SubAgentEventForwarded)
    );
}
