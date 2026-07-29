//! PermissionEngine — tool execution authorization.

use serde_json::Value as JsonValue;

/// Result of a permission check.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionResult {
    /// Execute immediately.
    Allow,
    /// Reject execution with a reason.
    Deny { reason: String },
    /// Require external confirmation before execution.
    RequireConfirm,
}

/// A single permission rule.
#[derive(Debug, Clone)]
pub struct PermissionRule {
    /// Glob or exact tool name pattern.
    pub tool_pattern: String,
    /// Whether to allow execution.
    pub allow: bool,
    /// Whether user confirmation is required.
    pub require_confirm: bool,
}

impl PermissionRule {
    /// Create a rule that allows a tool pattern.
    pub fn allow(pattern: impl Into<String>) -> Self {
        Self {
            tool_pattern: pattern.into(),
            allow: true,
            require_confirm: false,
        }
    }

    /// Create a rule that denies a tool pattern.
    pub fn deny(pattern: impl Into<String>) -> Self {
        Self {
            tool_pattern: pattern.into(),
            allow: false,
            require_confirm: false,
        }
    }
}

/// Tool execution authorization engine.
#[derive(Debug, Clone, Default)]
pub struct PermissionEngine {
    rules: Vec<PermissionRule>,
}

impl PermissionEngine {
    /// Create an empty engine (allows everything).
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a permission rule. Rules are checked in FIFO order.
    pub fn add_rule(&mut self, rule: PermissionRule) {
        self.rules.push(rule);
    }

    /// Check whether a tool call is permitted.
    ///
    /// # Matching
    /// Rules are checked in order. The first rule whose pattern matches
    /// the tool name wins. If no rule matches, the default is Allow.
    pub fn check(&self, tool_name: &str, _input: &JsonValue) -> PermissionResult {
        for rule in &self.rules {
            if matches_pattern(&rule.tool_pattern, tool_name) {
                if !rule.allow {
                    return PermissionResult::Deny {
                        reason: format!(
                            "tool '{}' denied by rule '{}'",
                            tool_name, rule.tool_pattern
                        ),
                    };
                }
                if rule.require_confirm {
                    return PermissionResult::RequireConfirm;
                }
                return PermissionResult::Allow;
            }
        }
        // Default: allow
        PermissionResult::Allow
    }
}

/// Simple pattern matching: exact match or wildcard "*" suffix.
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
        engine.add_rule(PermissionRule {
            tool_pattern: "expensive_*".into(),
            allow: true,
            require_confirm: true,
        });
        assert_eq!(
            engine.check("expensive_api", &serde_json::json!({})),
            PermissionResult::RequireConfirm
        );
    }

    #[test]
    fn test_first_match_wins() {
        let mut engine = PermissionEngine::new();
        engine.add_rule(PermissionRule::deny("tool"));
        engine.add_rule(PermissionRule::allow("tool"));
        assert!(matches!(
            engine.check("tool", &serde_json::json!({})),
            PermissionResult::Deny { .. }
        ));
    }
}
