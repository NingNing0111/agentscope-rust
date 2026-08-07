//! SubAgent templates, registered collaborators, and registry lookup.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agent_trait::Agent;
use crate::context_policy::{CapabilityScope, ContextSharingPolicy};
use crate::delegation::DelegationBudget;
use crate::subagent_error::SubAgentError;

/// Validation and availability status for a SubAgent template.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum TemplateStatus {
    #[default]
    Draft,
    Validated,
    Disabled,
    Invalid {
        reasons: Vec<String>,
    },
}

/// Runtime state for a registered SubAgent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentState {
    #[default]
    Configured,
    Selected,
    Running,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
    Disabled,
}

/// Rules for selecting a SubAgent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SelectionPolicy {
    #[default]
    ExplicitOnly,
    ResponsibilityMatch,
    ManualApprovalRequired,
}

/// Reusable SubAgent creation/configuration blueprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubAgentTemplate {
    pub template_id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    #[serde(default)]
    pub capability_scope: CapabilityScope,
    #[serde(default)]
    pub context_policy: ContextSharingPolicy,
    #[serde(default)]
    pub default_budget: DelegationBudget,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub status: TemplateStatus,
}

impl SubAgentTemplate {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        instructions: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            template_id: uuid::Uuid::new_v4().as_simple().to_string(),
            name,
            description: description.into(),
            instructions: instructions.into(),
            capability_scope: CapabilityScope::default(),
            context_policy: ContextSharingPolicy::default(),
            default_budget: DelegationBudget::default(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            status: TemplateStatus::Draft,
        }
    }

    pub fn validate(&self) -> Result<(), SubAgentError> {
        let mut reasons = Vec::new();
        if self.template_id.trim().is_empty() {
            reasons.push("template_id must not be empty");
        }
        if self.name.trim().is_empty() {
            reasons.push("name must not be empty");
        }
        if self.description.trim().is_empty() {
            reasons.push("description must not be empty");
        }
        if self.instructions.trim().is_empty() {
            reasons.push("instructions must not be empty");
        }
        if reasons.is_empty() {
            Ok(())
        } else {
            Err(SubAgentError::InvalidTemplate {
                reason: reasons.join("; "),
            })
        }
    }

    pub fn create_subagent(&self, agent: Arc<dyn Agent>) -> Result<SubAgent, SubAgentError> {
        self.validate()?;
        let agent_name = agent.name();
        if agent_name != self.name {
            return Err(SubAgentError::InvalidTemplate {
                reason: format!(
                    "template name '{}' does not match agent name '{}'",
                    self.name, agent_name
                ),
            });
        }
        Ok(SubAgent {
            agent_id: uuid::Uuid::new_v4().as_simple().to_string(),
            name: self.name.clone(),
            description: self.description.clone(),
            template_id: Some(self.template_id.clone()),
            state: SubAgentState::Configured,
            capability_scope: self.capability_scope.clone(),
            context_policy: self.context_policy.clone(),
            default_budget: self.default_budget.clone(),
            metadata: self.metadata.clone(),
            agent,
        })
    }
}

/// Registered in-process SubAgent collaborator.
///
/// `capability_scope` documents the target's intended permissions. At the
/// current opaque `Arc<dyn Agent>` delegation boundary the runtime can enforce
/// model access denial and fail closed for fully denied side effects before
/// invocation. Fine-grained tools, memory, session, workspace, and sandbox
/// restrictions must still be enforced cooperatively by the concrete child
/// agent/tool providers because this wrapper cannot introspect or mediate their
/// internal behavior.
#[derive(Clone)]
pub struct SubAgent {
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub template_id: Option<String>,
    pub state: SubAgentState,
    pub capability_scope: CapabilityScope,
    pub context_policy: ContextSharingPolicy,
    pub default_budget: DelegationBudget,
    pub metadata: serde_json::Value,
    pub agent: Arc<dyn Agent>,
}

impl std::fmt::Debug for SubAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubAgent")
            .field("agent_id", &self.agent_id)
            .field("name", &self.name)
            .field("description", &self.description)
            .field("template_id", &self.template_id)
            .field("state", &self.state)
            .field("capability_scope", &self.capability_scope)
            .field("context_policy", &self.context_policy)
            .field("default_budget", &self.default_budget)
            .field("metadata", &self.metadata)
            .finish_non_exhaustive()
    }
}

impl SubAgent {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        agent: Arc<dyn Agent>,
    ) -> Result<Self, SubAgentError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SubAgentError::InvalidTemplate {
                reason: "name must not be empty".to_string(),
            });
        }
        if agent.name() != name {
            return Err(SubAgentError::InvalidTemplate {
                reason: format!(
                    "subagent name '{}' does not match agent name '{}'",
                    name,
                    agent.name()
                ),
            });
        }
        Ok(Self {
            agent_id: uuid::Uuid::new_v4().as_simple().to_string(),
            name,
            description: description.into(),
            template_id: None,
            state: SubAgentState::Configured,
            capability_scope: CapabilityScope::default(),
            context_policy: ContextSharingPolicy::default(),
            default_budget: DelegationBudget::default(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            agent,
        })
    }

    pub fn enabled(&self) -> bool {
        self.state != SubAgentState::Disabled
    }
}

/// In-process registry of templates and concrete SubAgents.
#[derive(Debug, Clone)]
pub struct SubAgentRegistry {
    pub registry_id: String,
    pub parent_agent_name: String,
    pub selection_policy: SelectionPolicy,
    templates: HashMap<String, SubAgentTemplate>,
    subagents: HashMap<String, SubAgent>,
}

impl SubAgentRegistry {
    pub fn new(parent_agent_name: impl Into<String>) -> Self {
        Self {
            registry_id: uuid::Uuid::new_v4().as_simple().to_string(),
            parent_agent_name: parent_agent_name.into(),
            selection_policy: SelectionPolicy::ExplicitOnly,
            templates: HashMap::new(),
            subagents: HashMap::new(),
        }
    }

    pub fn register_template(
        &mut self,
        mut template: SubAgentTemplate,
    ) -> Result<(), SubAgentError> {
        template.validate()?;
        let key = normalize_name(&template.name);
        if self.templates.contains_key(&key) || self.subagents.contains_key(&key) {
            return Err(SubAgentError::DuplicateSubAgent {
                name: template.name,
            });
        }
        template.status = TemplateStatus::Validated;
        self.templates.insert(key, template);
        Ok(())
    }

    pub fn register_subagent(&mut self, subagent: SubAgent) -> Result<(), SubAgentError> {
        if subagent.name.trim().is_empty() {
            return Err(SubAgentError::InvalidTemplate {
                reason: "name must not be empty".to_string(),
            });
        }
        let key = normalize_name(&subagent.name);
        if self.subagents.contains_key(&key) || self.templates.contains_key(&key) {
            return Err(SubAgentError::DuplicateSubAgent {
                name: subagent.name,
            });
        }
        self.subagents.insert(key, subagent);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<&SubAgent, SubAgentError> {
        let key = normalize_name(name);
        let subagent = self
            .subagents
            .get(&key)
            .ok_or_else(|| SubAgentError::MissingSubAgent {
                name: name.to_string(),
            })?;
        if subagent.enabled() {
            Ok(subagent)
        } else {
            Err(SubAgentError::DisabledSubAgent {
                name: name.to_string(),
            })
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Result<&mut SubAgent, SubAgentError> {
        let key = normalize_name(name);
        let subagent =
            self.subagents
                .get_mut(&key)
                .ok_or_else(|| SubAgentError::MissingSubAgent {
                    name: name.to_string(),
                })?;
        if subagent.enabled() {
            Ok(subagent)
        } else {
            Err(SubAgentError::DisabledSubAgent {
                name: name.to_string(),
            })
        }
    }

    pub fn list(&self) -> Vec<&SubAgent> {
        let mut values: Vec<_> = self.subagents.values().collect();
        values.sort_by(|a, b| a.name.cmp(&b.name));
        values
    }

    pub fn enable(&mut self, name: &str) -> Result<(), SubAgentError> {
        let key = normalize_name(name);
        let subagent =
            self.subagents
                .get_mut(&key)
                .ok_or_else(|| SubAgentError::MissingSubAgent {
                    name: name.to_string(),
                })?;
        subagent.state = SubAgentState::Configured;
        Ok(())
    }

    pub fn disable(&mut self, name: &str) -> Result<(), SubAgentError> {
        let key = normalize_name(name);
        let subagent =
            self.subagents
                .get_mut(&key)
                .ok_or_else(|| SubAgentError::MissingSubAgent {
                    name: name.to_string(),
                })?;
        subagent.state = SubAgentState::Disabled;
        Ok(())
    }

    pub fn select(&self, query: Option<&str>, approved: bool) -> Result<&SubAgent, SubAgentError> {
        match self.selection_policy {
            SelectionPolicy::ExplicitOnly => {
                let name = query.ok_or_else(|| SubAgentError::InvalidDelegation {
                    reason: "explicit SubAgent target is required".to_string(),
                })?;
                self.get(name)
            }
            SelectionPolicy::ManualApprovalRequired if !approved => {
                Err(SubAgentError::InvalidDelegation {
                    reason: "manual approval is required before SubAgent selection".to_string(),
                })
            }
            SelectionPolicy::ManualApprovalRequired | SelectionPolicy::ResponsibilityMatch => {
                let query = query.unwrap_or_default().to_ascii_lowercase();
                let mut matches: Vec<&SubAgent> = self
                    .subagents
                    .values()
                    .filter(|agent| agent.enabled())
                    .filter(|agent| {
                        query.is_empty()
                            || agent.name.to_ascii_lowercase().contains(&query)
                            || agent.description.to_ascii_lowercase().contains(&query)
                    })
                    .collect();
                matches.sort_by(|a, b| a.name.cmp(&b.name));
                match matches.as_slice() {
                    [one] => Ok(*one),
                    [] => Err(SubAgentError::MissingSubAgent { name: query }),
                    _ => Err(SubAgentError::AmbiguousSubAgent { query }),
                }
            }
        }
    }
}

pub(crate) fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}
