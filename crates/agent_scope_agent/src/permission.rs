//! PermissionEngine — tool execution authorization.
//!
//! This module mirrors the Python AgentScope permission vocabulary while
//! preserving the Rust crate's original default behavior: an empty/default
//! engine allows tool calls unless an explicit deny/ask rule matches.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Permission modes controlling tool execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Ask by explicit rule; otherwise keep the Rust default of allowing calls.
    #[default]
    Default,
    /// Allow edits by default for now; explicit deny/ask rules still apply.
    AcceptEdits,
    /// Read-only planning mode. MVP behavior denies calls unless allowed by rule.
    Explore,
    /// Fully trusted mode. Explicit deny/ask rules still apply.
    Bypass,
    /// Unattended mode. Any ask decision is converted to deny.
    DontAsk,
}

/// Behavior selected by a permission rule or tool-level permission decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
    Passthrough,
}

/// Result of a permission check.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionResult {
    /// Execute immediately.
    Allow,
    /// Reject execution with a reason.
    Deny { reason: String },
    /// Require external confirmation before execution.
    ///
    /// The engine never pauses or resumes tool execution internally. Callers must
    /// emit a confirmation event, reject the current tool result, and retry the
    /// tool call only after an upper layer has approved it.
    RequireConfirm,
}

/// A directory that participates in permission decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionalWorkingDirectory {
    pub path: String,
    pub source: String,
}

/// A single permission rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Tool name or pattern this rule applies to. Supports exact, `*`, and suffix `*` prefix match.
    pub tool_name: String,
    /// Optional tool-specific match content. The MVP matches this as a substring
    /// against the serialized tool input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
    /// Rule behavior.
    pub behavior: PermissionBehavior,
    /// Where the rule came from, e.g. userSettings/projectSettings/session.
    pub source: String,
}

impl PermissionRule {
    /// Create a rule that allows a tool pattern.
    pub fn allow(pattern: impl Into<String>) -> Self {
        Self {
            tool_name: pattern.into(),
            rule_content: None,
            behavior: PermissionBehavior::Allow,
            source: "runtime".into(),
        }
    }

    /// Create a rule that denies a tool pattern.
    pub fn deny(pattern: impl Into<String>) -> Self {
        Self {
            tool_name: pattern.into(),
            rule_content: None,
            behavior: PermissionBehavior::Deny,
            source: "runtime".into(),
        }
    }

    /// Create a rule that requires user confirmation for a tool pattern.
    pub fn ask(pattern: impl Into<String>) -> Self {
        Self {
            tool_name: pattern.into(),
            rule_content: None,
            behavior: PermissionBehavior::Ask,
            source: "runtime".into(),
        }
    }

    /// Attach tool-specific rule content.
    pub fn with_rule_content(mut self, content: impl Into<String>) -> Self {
        self.rule_content = Some(content.into());
        self
    }

    /// Attach a rule source.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }
}

/// Context for permission checking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionContext {
    pub mode: PermissionMode,
    #[serde(default)]
    pub working_directories: HashMap<String, AdditionalWorkingDirectory>,
    #[serde(default)]
    pub allow_rules: HashMap<String, Vec<PermissionRule>>,
    #[serde(default)]
    pub deny_rules: HashMap<String, Vec<PermissionRule>>,
    #[serde(default)]
    pub ask_rules: HashMap<String, Vec<PermissionRule>>,
}

impl Default for PermissionContext {
    fn default() -> Self {
        Self {
            mode: PermissionMode::Default,
            working_directories: HashMap::new(),
            allow_rules: HashMap::new(),
            deny_rules: HashMap::new(),
            ask_rules: HashMap::new(),
        }
    }
}

impl PermissionContext {
    pub fn new(mode: PermissionMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    pub fn add_rule(&mut self, rule: PermissionRule) {
        let target = match rule.behavior {
            PermissionBehavior::Allow => &mut self.allow_rules,
            PermissionBehavior::Deny => &mut self.deny_rules,
            PermissionBehavior::Ask => &mut self.ask_rules,
            PermissionBehavior::Passthrough => return,
        };
        target.entry(rule.tool_name.clone()).or_default().push(rule);
    }
}

/// Decision returned by the permission engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub behavior: PermissionBehavior,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_rules: Option<Vec<PermissionRule>>,
    #[serde(default)]
    pub bypass_immune: bool,
}

impl PermissionDecision {
    pub fn allow(tool_name: &str, reason: impl Into<String>) -> Self {
        Self {
            behavior: PermissionBehavior::Allow,
            message: format!("Permission granted for {tool_name}"),
            decision_reason: Some(reason.into()),
            updated_input: None,
            suggested_rules: None,
            bypass_immune: false,
        }
    }

    pub fn deny(_tool_name: &str, message: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            behavior: PermissionBehavior::Deny,
            message: message.into(),
            decision_reason: Some(reason.into()),
            updated_input: None,
            suggested_rules: None,
            bypass_immune: false,
        }
    }

    pub fn ask(tool_name: &str, reason: impl Into<String>) -> Self {
        Self {
            behavior: PermissionBehavior::Ask,
            message: format!("Permission required for {tool_name}"),
            decision_reason: Some(reason.into()),
            updated_input: None,
            suggested_rules: None,
            bypass_immune: false,
        }
    }
}

/// Tool execution authorization engine.
#[derive(Debug, Clone, Default)]
pub struct PermissionEngine {
    context: PermissionContext,
}

impl PermissionEngine {
    /// Create an empty engine (allows everything unless configured otherwise).
    pub fn new() -> Self {
        Self {
            context: PermissionContext::default(),
        }
    }

    pub fn with_context(context: PermissionContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &PermissionContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut PermissionContext {
        &mut self.context
    }

    /// Add a permission rule.
    pub fn add_rule(&mut self, rule: PermissionRule) {
        self.context.add_rule(rule);
    }

    /// Check whether a tool call is permitted, returning the legacy result type.
    pub fn check(&self, tool_name: &str, input: &JsonValue) -> PermissionResult {
        match self.check_decision(tool_name, input).behavior {
            PermissionBehavior::Allow | PermissionBehavior::Passthrough => PermissionResult::Allow,
            PermissionBehavior::Deny => PermissionResult::Deny {
                reason: self.check_decision(tool_name, input).message,
            },
            PermissionBehavior::Ask => PermissionResult::RequireConfirm,
        }
    }

    /// Check whether a tool call is permitted.
    pub fn check_decision(&self, tool_name: &str, input: &JsonValue) -> PermissionDecision {
        let input_text = input.to_string();

        if let Some(rule) = find_matching_rule(&self.context.deny_rules, tool_name, &input_text) {
            return PermissionDecision::deny(
                tool_name,
                format!("tool '{tool_name}' denied by rule '{}'", rule.tool_name),
                format!("deny rule from {}", rule.source),
            );
        }

        if let Some(rule) = find_matching_rule(&self.context.ask_rules, tool_name, &input_text) {
            let ask = PermissionDecision {
                suggested_rules: Some(vec![PermissionRule::allow(tool_name)]),
                ..PermissionDecision::ask(tool_name, format!("ask rule from {}", rule.source))
            };
            return if self.context.mode == PermissionMode::DontAsk {
                Self::convert_ask_to_deny(tool_name, ask)
            } else {
                ask
            };
        }

        if let Some(rule) = find_matching_rule(&self.context.allow_rules, tool_name, &input_text) {
            return PermissionDecision::allow(
                tool_name,
                format!("allow rule from {}", rule.source),
            );
        }

        // Built-in task planning tools only read/write the agent's own task
        // state. They bypass restrictive mode defaults, but explicit rules above
        // still apply; in particular, explicit deny remains the highest priority.
        if crate::task_tools::TASK_TOOL_NAMES.contains(&tool_name) {
            return PermissionDecision::allow(
                tool_name,
                format!("{tool_name} is allowed as a built-in task tool."),
            );
        }

        match self.context.mode {
            PermissionMode::Explore => PermissionDecision::deny(
                tool_name,
                format!("Permission denied for {tool_name} (explore mode is read-only)"),
                "Explore mode does not allow unclassified tool calls",
            ),
            PermissionMode::DontAsk => {
                PermissionDecision::allow(tool_name, "dont_ask default allow")
            }
            PermissionMode::Default | PermissionMode::AcceptEdits | PermissionMode::Bypass => {
                PermissionDecision::allow(tool_name, format!("Mode: {:?}", self.context.mode))
            }
        }
    }

    fn convert_ask_to_deny(tool_name: &str, ask: PermissionDecision) -> PermissionDecision {
        PermissionDecision {
            behavior: PermissionBehavior::Deny,
            message: format!(
                "Permission denied for {tool_name}: user confirmation required but permission mode is dont_ask"
            ),
            decision_reason: ask.decision_reason,
            updated_input: ask.updated_input,
            suggested_rules: ask.suggested_rules,
            bypass_immune: ask.bypass_immune,
        }
    }
}

fn find_matching_rule<'a>(
    rules: &'a HashMap<String, Vec<PermissionRule>>,
    tool_name: &str,
    input_text: &str,
) -> Option<&'a PermissionRule> {
    rules
        .iter()
        .filter(|(pattern, _)| matches_pattern(pattern, tool_name))
        .flat_map(|(_, rules)| rules.iter())
        .find(|rule| rule_matches(rule, tool_name, input_text))
}

fn rule_matches(rule: &PermissionRule, tool_name: &str, input_text: &str) -> bool {
    matches_pattern(&rule.tool_name, tool_name)
        && rule
            .rule_content
            .as_ref()
            .is_none_or(|content| input_text.contains(content))
}

/// Simple pattern matching: exact match or wildcard `*` suffix.
fn matches_pattern(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    pattern == name
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_engine_allows_all() {
        let engine = PermissionEngine::new();
        assert_eq!(
            engine.check("any_tool", &serde_json::json!({})),
            PermissionResult::Allow
        );
    }

    #[test]
    fn test_deny_rule() {
        let mut engine = PermissionEngine::new();
        engine.add_rule(PermissionRule::deny("dangerous_tool"));
        assert_eq!(
            engine.check("dangerous_tool", &serde_json::json!({})),
            PermissionResult::Deny {
                reason: "tool 'dangerous_tool' denied by rule 'dangerous_tool'".into()
            }
        );
    }

    #[test]
    fn test_wildcard_pattern() {
        let mut engine = PermissionEngine::new();
        engine.add_rule(PermissionRule::deny("file_*"));
        assert!(matches!(
            engine.check("file_read", &serde_json::json!({})),
            PermissionResult::Deny { .. }
        ));
        assert!(matches!(
            engine.check("file_write", &serde_json::json!({})),
            PermissionResult::Deny { .. }
        ));
        assert_eq!(
            engine.check("other_tool", &serde_json::json!({})),
            PermissionResult::Allow
        );
    }

    #[test]
    fn test_require_confirm() {
        let mut engine = PermissionEngine::new();
        engine.add_rule(PermissionRule::ask("expensive_*"));
        assert_eq!(
            engine.check("expensive_api", &serde_json::json!({})),
            PermissionResult::RequireConfirm
        );
    }

    #[test]
    fn test_deny_precedes_allow() {
        let mut engine = PermissionEngine::new();
        engine.add_rule(PermissionRule::allow("tool"));
        engine.add_rule(PermissionRule::deny("tool"));
        assert!(matches!(
            engine.check("tool", &serde_json::json!({})),
            PermissionResult::Deny { .. }
        ));
    }

    #[test]
    fn test_rule_content_matches_input() {
        let mut engine = PermissionEngine::new();
        engine.add_rule(PermissionRule::ask("shell").with_rule_content("rm -rf"));
        assert_eq!(
            engine.check("shell", &serde_json::json!({ "cmd": "rm -rf /tmp/x" })),
            PermissionResult::RequireConfirm
        );
        assert_eq!(
            engine.check("shell", &serde_json::json!({ "cmd": "ls" })),
            PermissionResult::Allow
        );
    }

    #[test]
    fn test_dont_ask_converts_ask_to_deny() {
        let mut context = PermissionContext::new(PermissionMode::DontAsk);
        context.add_rule(PermissionRule::ask("dangerous_tool"));
        let engine = PermissionEngine::with_context(context);
        let decision = engine.check_decision("dangerous_tool", &serde_json::json!({}));
        assert_eq!(decision.behavior, PermissionBehavior::Deny);
        assert!(decision.message.contains("dont_ask"));
    }

    #[test]
    fn test_explore_denies_unclassified_tools() {
        let engine =
            PermissionEngine::with_context(PermissionContext::new(PermissionMode::Explore));
        let decision = engine.check_decision("write", &serde_json::json!({}));
        assert_eq!(decision.behavior, PermissionBehavior::Deny);
    }

    #[test]
    fn test_deny_rule_overrides_task_tool_auto_allow() {
        let mut context = PermissionContext::new(PermissionMode::Default);
        context.add_rule(PermissionRule::deny("TaskCreate"));
        let engine = PermissionEngine::with_context(context);

        let decision = engine.check_decision("TaskCreate", &serde_json::json!({}));
        assert_eq!(decision.behavior, PermissionBehavior::Deny);
    }

    #[test]
    fn test_task_tools_bypass_mode_defaults_but_not_explicit_rules() {
        for mode in [PermissionMode::Explore, PermissionMode::DontAsk] {
            let engine = PermissionEngine::with_context(PermissionContext::new(mode));
            let decision = engine.check_decision("TaskList", &serde_json::json!({}));
            assert_eq!(
                decision.behavior,
                PermissionBehavior::Allow,
                "mode {mode:?}"
            );
        }

        let mut context = PermissionContext::new(PermissionMode::Explore);
        context.add_rule(PermissionRule::ask("TaskList"));
        let engine = PermissionEngine::with_context(context);
        let decision = engine.check_decision("TaskList", &serde_json::json!({}));
        assert_eq!(decision.behavior, PermissionBehavior::Ask);

        let mut context = PermissionContext::new(PermissionMode::DontAsk);
        context.add_rule(PermissionRule::ask("TaskList"));
        let engine = PermissionEngine::with_context(context);
        let decision = engine.check_decision("TaskList", &serde_json::json!({}));
        assert_eq!(decision.behavior, PermissionBehavior::Deny);
        assert!(decision.message.contains("dont_ask"));
    }
}
