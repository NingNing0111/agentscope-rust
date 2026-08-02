#[path = "support/subagent_test_agent.rs"]
mod subagent_test_agent;

use agent_scope_agent::*;
use subagent_test_agent::scripted_agent;

fn subagent(name: &str, response: &str) -> SubAgent {
    SubAgent::new(
        name,
        format!("{name} responsibility"),
        scripted_agent(name, response),
    )
    .unwrap()
}

#[test]
fn subagent_foundational_types_serde_roundtrip() {
    let template = SubAgentTemplate::new("researcher", "research", "find facts");
    let json = serde_json::to_string(&template).unwrap();
    let decoded: SubAgentTemplate = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.name, "researcher");
    assert_eq!(
        CollaborationStatus::Succeeded,
        serde_json::from_str("\"succeeded\"").unwrap()
    );
}

#[test]
fn template_validation_success_and_failure() {
    let template = SubAgentTemplate::new("researcher", "research", "find facts");
    assert!(template.validate().is_ok());
    let mut trace = DelegationTrace::new("reply", "delegation", "planner", "researcher");
    trace.append(
        DelegationEventType::TemplateValidated,
        "planner",
        "api_key=secret should redact",
    );
    assert!(trace.has_event(DelegationEventType::TemplateValidated));
    assert!(!trace.events[0].summary.contains("secret"));

    let invalid = SubAgentTemplate::new("", "", "");
    let err = invalid.validate().unwrap_err();
    assert_eq!(err.category(), SubAgentErrorCategory::InvalidTemplate);
}

#[test]
fn registry_register_list_lookup_enable_disable() {
    let mut registry = SubAgentRegistry::new("planner");
    registry
        .register_subagent(subagent("researcher", "facts"))
        .unwrap();
    assert_eq!(registry.list()[0].name, "researcher");
    assert!(registry.get("RESEARCHER").is_ok());
    assert_eq!(
        registry
            .register_subagent(subagent("researcher", "facts"))
            .unwrap_err()
            .category(),
        SubAgentErrorCategory::DuplicateSubAgent
    );
    registry.disable("researcher").unwrap();
    assert_eq!(
        registry.get("researcher").unwrap_err().category(),
        SubAgentErrorCategory::DisabledSubAgent
    );
    registry.enable("researcher").unwrap();
    assert!(registry.get("researcher").is_ok());
}
