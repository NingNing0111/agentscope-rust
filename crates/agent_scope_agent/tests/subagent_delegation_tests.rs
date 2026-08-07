#[path = "support/subagent_test_agent.rs"]
mod subagent_test_agent;

use agent_scope_agent::*;
use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};
use subagent_test_agent::{ScriptedTestAgent, scripted_agent};

fn text_msg(name: &str, text: &str, role: Role) -> Msg {
    Msg::new(
        name.to_string(),
        vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
        role,
    )
    .unwrap()
}

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

#[tokio::test]
async fn target_default_budget_is_not_relaxed_by_request() {
    let researcher = scripted_agent("researcher", "ok");
    let mut subagent = SubAgent::new("researcher", "research", researcher.clone()).unwrap();
    subagent.default_budget.timeout_ms = 1_000;
    subagent.default_budget.max_context_messages = 1;
    subagent.context_policy.message_policy = MessageContextPolicy::Full { explicit: true };

    let mut registry = SubAgentRegistry::new("planner");
    registry.register_subagent(subagent).unwrap();

    let mut request = DelegationRequest::new("planner", "researcher", "task");
    request.budget.timeout_ms = 30_000;
    request.budget.max_context_messages = 10;
    request.context.messages = vec![
        text_msg("user", "one", Role::User),
        text_msg("assistant", "two", Role::Assistant),
    ];

    let result = delegate_once(&registry, request).await.unwrap();
    assert_eq!(result.status, CollaborationStatus::Failed);
    assert_eq!(
        result.error.unwrap().category,
        SubAgentErrorCategory::BudgetExceeded
    );
    assert!(researcher.received().is_empty());
}

#[tokio::test]
async fn target_context_policy_sanitizes_direct_shared_context() {
    let researcher = scripted_agent("researcher", "ok");
    let mut subagent = SubAgent::new("researcher", "research", researcher.clone()).unwrap();
    subagent.context_policy.message_policy = MessageContextPolicy::None;

    let mut registry = SubAgentRegistry::new("planner");
    registry.register_subagent(subagent).unwrap();

    let mut request = DelegationRequest::new("planner", "researcher", "task");
    request.context.messages = vec![text_msg("user", "must-not-forward", Role::User)];

    let result = delegate_once(&registry, request).await.unwrap();
    assert_eq!(result.status, CollaborationStatus::Succeeded);
    let received = researcher.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].len(), 1);
    assert_eq!(received[0][0].name, "planner");
}

#[tokio::test]
async fn target_model_access_denied_blocks_delegation() {
    let researcher = scripted_agent("researcher", "ok");
    let mut subagent = SubAgent::new("researcher", "research", researcher.clone()).unwrap();
    subagent.capability_scope.model_access = ModelAccessPolicy::Denied;

    let mut registry = SubAgentRegistry::new("planner");
    registry.register_subagent(subagent).unwrap();

    let result = delegate_once(
        &registry,
        DelegationRequest::new("planner", "researcher", "task"),
    )
    .await
    .unwrap();
    assert_eq!(result.status, CollaborationStatus::PermissionDenied);
    assert_eq!(
        result.error.unwrap().category,
        SubAgentErrorCategory::PermissionDenied
    );
    assert!(researcher.received().is_empty());
}

#[tokio::test]
async fn denied_side_effect_scope_fails_closed_for_opaque_subagent() {
    let researcher = scripted_agent("researcher", "ok");
    let mut subagent = SubAgent::new("researcher", "research", researcher.clone()).unwrap();
    subagent.capability_scope.side_effects = SideEffectPolicy::Denied;

    let mut registry = SubAgentRegistry::new("planner");
    registry.register_subagent(subagent).unwrap();

    let result = delegate_once(
        &registry,
        DelegationRequest::new("planner", "researcher", "task"),
    )
    .await
    .unwrap();
    assert_eq!(result.status, CollaborationStatus::PermissionDenied);
    assert!(researcher.received().is_empty());
}
