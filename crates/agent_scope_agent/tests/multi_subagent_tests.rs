#[path = "support/subagent_test_agent.rs"]
mod subagent_test_agent;

use agent_scope_agent::*;
use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};
use subagent_test_agent::scripted_agent;

fn subagent(name: &str, response: &str) -> SubAgent {
    SubAgent::new(
        name,
        format!("{name} responsibility"),
        scripted_agent(name, response),
    )
    .unwrap()
}

fn text_msg(name: &str, text: &str, role: Role) -> Msg {
    Msg::new(
        name.to_string(),
        vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
        role,
    )
    .unwrap()
}

#[tokio::test]
async fn two_subagent_registration_and_distinct_tasks() {
    let researcher = scripted_agent("researcher", "facts");
    let writer = scripted_agent("writer", "summary");
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(SubAgent::new("researcher", "research", researcher.clone()).unwrap())
        .unwrap();
    registry
        .register_subagent(SubAgent::new("writer", "writing", writer.clone()).unwrap())
        .unwrap();
    let results = delegate_many(
        &registry,
        vec![
            DelegationRequest::new("planner", "researcher", "collect facts"),
            DelegationRequest::new("planner", "writer", "write summary"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(results.len(), 2);
    assert!(
        researcher.received()[0]
            .last()
            .unwrap()
            .get_text_content("")
            .unwrap()
            .contains("collect facts")
    );
    assert!(
        writer.received()[0]
            .last()
            .unwrap()
            .get_text_content("")
            .unwrap()
            .contains("write summary")
    );
}

#[test]
fn multi_agent_conversation_preserves_speaker_identity() {
    let mut conv = MultiAgentConversation::new("conv-1");
    conv.add_participant("user", "user");
    conv.add_participant("planner", "parent");
    conv.add_participant("researcher", "subagent");
    conv.add_participant("writer", "subagent");
    conv.push_message(text_msg("researcher", "facts", Role::Assistant));
    conv.push_message(text_msg("writer", "summary", Role::Assistant));
    assert_eq!(
        conv.messages
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        vec!["researcher", "writer"]
    );
}

#[tokio::test]
async fn unselected_collaborator_is_not_invoked() {
    let researcher = scripted_agent("researcher", "facts");
    let writer = scripted_agent("writer", "summary");
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(SubAgent::new("researcher", "research", researcher.clone()).unwrap())
        .unwrap();
    registry
        .register_subagent(SubAgent::new("writer", "writing", writer.clone()).unwrap())
        .unwrap();
    delegate_once(
        &registry,
        DelegationRequest::new("planner", "researcher", "collect facts"),
    )
    .await
    .unwrap();
    assert_eq!(researcher.received().len(), 1);
    assert!(writer.received().is_empty());
}

#[test]
fn selection_policy_reports_ambiguous_or_unapproved() {
    let mut registry = SubAgentRegistry::new("planner");
    registry.selection_policy = SelectionPolicy::ResponsibilityMatch;
    registry
        .register_subagent(subagent("researcher", "analysis"))
        .unwrap();
    registry
        .register_subagent(subagent("writer", "analysis"))
        .unwrap();
    assert_eq!(
        registry
            .select(Some("responsibility"), true)
            .unwrap_err()
            .category(),
        SubAgentErrorCategory::AmbiguousSubAgent
    );
    registry.selection_policy = SelectionPolicy::ManualApprovalRequired;
    assert_eq!(
        registry
            .select(Some("analysis"), false)
            .unwrap_err()
            .category(),
        SubAgentErrorCategory::InvalidDelegation
    );
}
