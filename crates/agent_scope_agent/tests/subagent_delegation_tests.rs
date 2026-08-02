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

#[tokio::test]
async fn successful_single_delegation_preserves_attribution() {
    let researcher = scripted_agent("researcher", "deterministic facts");
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(SubAgent::new("researcher", "research", researcher.clone()).unwrap())
        .unwrap();
    let mut request = DelegationRequest::new("planner", "researcher", "collect facts");
    request.delegation_id = "delegation-1".into();
    let result = delegate_once(&registry, request).await.unwrap();
    assert_eq!(result.status, CollaborationStatus::Succeeded);
    assert_eq!(result.message.as_ref().unwrap().name, "researcher");
    assert_eq!(researcher.received()[0].last().unwrap().name, "planner");
}

#[tokio::test]
async fn parent_observes_successful_subagent_message() {
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
    assert!(
        result
            .trace
            .has_event(DelegationEventType::ResultObservedByParent)
    );
    assert_eq!(parent.received()[0][0].name, "researcher");
}
