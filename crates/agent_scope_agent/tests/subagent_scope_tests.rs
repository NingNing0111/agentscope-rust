#[path = "support/subagent_test_agent.rs"]
mod subagent_test_agent;

use agent_scope_agent::*;
use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};

fn text_msg(name: &str, text: &str, role: Role) -> Msg {
    Msg::new(
        name.to_string(),
        vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
        role,
    )
    .unwrap()
}

#[test]
fn context_policy_message_sharing_modes() {
    let messages = vec![
        text_msg("user", "one", Role::User),
        text_msg("planner", "two", Role::Assistant),
    ];
    let none = ContextSharingPolicy::default()
        .build_shared_context(&messages, Some("summary".into()))
        .unwrap();
    assert!(none.messages.is_empty());
    let mut policy = ContextSharingPolicy {
        message_policy: MessageContextPolicy::SummaryOnly,
        ..Default::default()
    };
    assert_eq!(
        policy
            .build_shared_context(&messages, Some("summary".into()))
            .unwrap()
            .messages
            .len(),
        1
    );
    policy.message_policy = MessageContextPolicy::Selected {
        message_ids: vec![messages[0].id.clone()],
    };
    assert_eq!(
        policy
            .build_shared_context(&messages, None)
            .unwrap()
            .messages[0]
            .name,
        "user"
    );
    policy.message_policy = MessageContextPolicy::Full { explicit: true };
    assert_eq!(
        policy
            .build_shared_context(&messages, None)
            .unwrap()
            .messages
            .len(),
        2
    );
    policy.message_policy = MessageContextPolicy::Full { explicit: false };
    assert_eq!(
        policy
            .build_shared_context(&messages, None)
            .unwrap_err()
            .category(),
        SubAgentErrorCategory::PermissionDenied
    );
}

#[test]
fn capability_denial_for_resources_and_tools() {
    let scope = CapabilityScope::default();
    assert_eq!(
        scope.require_tool("bash").unwrap_err().category(),
        SubAgentErrorCategory::PermissionDenied
    );
    assert_eq!(
        scope.require_memory().unwrap_err().category(),
        SubAgentErrorCategory::PermissionDenied
    );
    assert_eq!(
        scope.require_session().unwrap_err().category(),
        SubAgentErrorCategory::PermissionDenied
    );
    assert_eq!(
        scope.require_workspace().unwrap_err().category(),
        SubAgentErrorCategory::PermissionDenied
    );
    assert_eq!(
        scope.require_sandbox().unwrap_err().category(),
        SubAgentErrorCategory::PermissionDenied
    );
}

#[test]
fn side_effect_records_are_attributed_and_redacted() {
    let record = SideEffectRecord {
        effect_id: "e1".into(),
        subagent_name: "researcher".into(),
        effect_type: SideEffectType::ToolInvocation,
        scope: "subagent-only".into(),
        summary: safe_summary("password=hunter2"),
    };
    assert_eq!(record.subagent_name, "researcher");
    assert!(!record.summary.contains("hunter2"));
}

#[test]
fn delegation_budget_validation() {
    let mut request = DelegationRequest::new("planner", "researcher", "task");
    request.budget.max_depth = 0;
    assert_eq!(
        request.validate().unwrap_err().category(),
        SubAgentErrorCategory::BudgetExceeded
    );
    request.budget.max_depth = 1;
    request.budget.max_calls = 0;
    assert_eq!(
        request.validate().unwrap_err().category(),
        SubAgentErrorCategory::BudgetExceeded
    );
}
