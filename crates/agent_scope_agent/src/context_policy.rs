//! Context sharing and capability scope for SubAgent delegation.

use serde::{Deserialize, Serialize};

use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};

use crate::subagent_error::SubAgentError;

/// Controls which parent messages are shared with a SubAgent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MessageContextPolicy {
    #[default]
    None,
    SummaryOnly,
    Selected {
        message_ids: Vec<String>,
    },
    Full {
        explicit: bool,
    },
}

/// Controls access to shared resources.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ResourceSharingPolicy {
    #[default]
    None,
    ReadOnly,
    Scoped {
        refs: Vec<String>,
    },
    Inherited {
        explicit: bool,
    },
}

/// Controls model usage by a SubAgent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAccessPolicy {
    Denied,
    #[default]
    SameAsParent,
    Dedicated,
}

/// Controls whether persistent side effects are allowed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectPolicy {
    Denied,
    #[default]
    SubAgentScoped,
    ParentPromoted,
}

/// Effective capabilities available to a SubAgent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityScope {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub memory: ResourceSharingPolicy,
    #[serde(default)]
    pub session: ResourceSharingPolicy,
    #[serde(default)]
    pub workspace: ResourceSharingPolicy,
    #[serde(default)]
    pub sandbox: ResourceSharingPolicy,
    #[serde(default)]
    pub model_access: ModelAccessPolicy,
    #[serde(default)]
    pub side_effects: SideEffectPolicy,
}

impl Default for CapabilityScope {
    fn default() -> Self {
        Self {
            tools: Vec::new(),
            memory: ResourceSharingPolicy::None,
            session: ResourceSharingPolicy::None,
            workspace: ResourceSharingPolicy::None,
            sandbox: ResourceSharingPolicy::None,
            model_access: ModelAccessPolicy::SameAsParent,
            side_effects: SideEffectPolicy::SubAgentScoped,
        }
    }
}

impl CapabilityScope {
    pub fn allows_tool(&self, tool_name: &str) -> bool {
        self.tools.iter().any(|t| t == "*" || t == tool_name)
    }

    pub fn require_tool(&self, tool_name: &str) -> Result<(), SubAgentError> {
        if self.allows_tool(tool_name) {
            Ok(())
        } else {
            Err(SubAgentError::PermissionDenied {
                capability: format!("tool:{tool_name}"),
                reason: "tool is outside the SubAgent capability scope".to_string(),
            })
        }
    }

    pub fn require_resource(
        &self,
        resource: &str,
        policy: &ResourceSharingPolicy,
    ) -> Result<(), SubAgentError> {
        match policy {
            ResourceSharingPolicy::None => Err(SubAgentError::PermissionDenied {
                capability: resource.to_string(),
                reason: "resource is not shared with this SubAgent".to_string(),
            }),
            ResourceSharingPolicy::ReadOnly
            | ResourceSharingPolicy::Scoped { .. }
            | ResourceSharingPolicy::Inherited { explicit: true } => Ok(()),
            ResourceSharingPolicy::Inherited { explicit: false } => {
                Err(SubAgentError::PermissionDenied {
                    capability: resource.to_string(),
                    reason: "inherited access requires explicit opt-in".to_string(),
                })
            }
        }
    }

    pub fn require_memory(&self) -> Result<(), SubAgentError> {
        self.require_resource("memory", &self.memory)
    }

    pub fn require_session(&self) -> Result<(), SubAgentError> {
        self.require_resource("session", &self.session)
    }

    pub fn require_workspace(&self) -> Result<(), SubAgentError> {
        self.require_resource("workspace", &self.workspace)
    }

    pub fn require_sandbox(&self) -> Result<(), SubAgentError> {
        self.require_resource("sandbox", &self.sandbox)
    }

    pub fn require_model(&self) -> Result<(), SubAgentError> {
        match self.model_access {
            ModelAccessPolicy::Denied => Err(SubAgentError::PermissionDenied {
                capability: "model".to_string(),
                reason: "model access is denied for this SubAgent".to_string(),
            }),
            ModelAccessPolicy::SameAsParent | ModelAccessPolicy::Dedicated => Ok(()),
        }
    }

    pub fn require_side_effects(&self) -> Result<(), SubAgentError> {
        match self.side_effects {
            SideEffectPolicy::Denied => Err(SubAgentError::PermissionDenied {
                capability: "side_effects".to_string(),
                reason: "persistent side effects are denied for this SubAgent".to_string(),
            }),
            SideEffectPolicy::SubAgentScoped | SideEffectPolicy::ParentPromoted => Ok(()),
        }
    }
}

/// Policy controlling messages/resources shared with a SubAgent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSharingPolicy {
    #[serde(default)]
    pub message_policy: MessageContextPolicy,
    #[serde(default)]
    pub memory_policy: ResourceSharingPolicy,
    #[serde(default)]
    pub session_policy: ResourceSharingPolicy,
    #[serde(default)]
    pub workspace_policy: ResourceSharingPolicy,
    #[serde(default)]
    pub tool_policy: ResourceSharingPolicy,
    #[serde(default)]
    pub promote_results_to_parent: bool,
}

impl Default for ContextSharingPolicy {
    fn default() -> Self {
        Self {
            message_policy: MessageContextPolicy::None,
            memory_policy: ResourceSharingPolicy::None,
            session_policy: ResourceSharingPolicy::None,
            workspace_policy: ResourceSharingPolicy::None,
            tool_policy: ResourceSharingPolicy::None,
            promote_results_to_parent: false,
        }
    }
}

/// Context explicitly shared with a SubAgent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedContext {
    #[serde(default)]
    pub messages: Vec<Msg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub memory_refs: Vec<String>,
    #[serde(default)]
    pub session_refs: Vec<String>,
    #[serde(default)]
    pub workspace_refs: Vec<String>,
    #[serde(default)]
    pub redaction_notes: Vec<String>,
}

impl SharedContext {
    pub fn empty() -> Self {
        Self {
            messages: Vec::new(),
            summary: None,
            memory_refs: Vec::new(),
            session_refs: Vec::new(),
            workspace_refs: Vec::new(),
            redaction_notes: Vec::new(),
        }
    }
}

impl ContextSharingPolicy {
    pub fn build_shared_context(
        &self,
        source_messages: &[Msg],
        summary: Option<String>,
    ) -> Result<SharedContext, SubAgentError> {
        let mut ctx = SharedContext::empty();
        match &self.message_policy {
            MessageContextPolicy::None => {}
            MessageContextPolicy::SummaryOnly => {
                ctx.summary = summary.clone();
                if let Some(text) = summary {
                    ctx.messages.push(summary_msg(&text));
                }
            }
            MessageContextPolicy::Selected { message_ids } => {
                ctx.messages = source_messages
                    .iter()
                    .filter(|m| message_ids.contains(&m.id))
                    .cloned()
                    .collect();
            }
            MessageContextPolicy::Full { explicit } => {
                if !explicit {
                    return Err(SubAgentError::PermissionDenied {
                        capability: "message_context:full".to_string(),
                        reason: "full context sharing requires explicit opt-in".to_string(),
                    });
                }
                ctx.messages = source_messages.to_vec();
            }
        }
        Ok(ctx)
    }

    /// Sanitize caller-supplied context so it cannot exceed this SubAgent's
    /// sharing policy. Delegation accepts `SharedContext` for compatibility, but
    /// callers must not be able to bypass `message_policy` by constructing it
    /// directly.
    pub fn sanitize_shared_context(
        &self,
        context: &SharedContext,
    ) -> Result<SharedContext, SubAgentError> {
        let mut sanitized = SharedContext::empty();
        sanitized.summary = context.summary.clone();
        sanitized.memory_refs = sanitize_refs("memory", &self.memory_policy, &context.memory_refs)?;
        sanitized.session_refs =
            sanitize_refs("session", &self.session_policy, &context.session_refs)?;
        sanitized.workspace_refs =
            sanitize_refs("workspace", &self.workspace_policy, &context.workspace_refs)?;
        sanitized.redaction_notes = context.redaction_notes.clone();

        match &self.message_policy {
            MessageContextPolicy::None => {
                if !context.messages.is_empty() {
                    sanitized
                        .redaction_notes
                        .push("message context removed by policy:none".to_string());
                }
            }
            MessageContextPolicy::SummaryOnly => {
                if let Some(summary) = &context.summary {
                    sanitized.messages.push(summary_msg(summary));
                }
                if !context.messages.is_empty() {
                    sanitized
                        .redaction_notes
                        .push("raw messages removed by policy:summary_only".to_string());
                }
            }
            MessageContextPolicy::Selected { message_ids } => {
                sanitized.messages = context
                    .messages
                    .iter()
                    .filter(|message| message_ids.contains(&message.id))
                    .cloned()
                    .collect();
            }
            MessageContextPolicy::Full { explicit } => {
                if !explicit {
                    return Err(SubAgentError::PermissionDenied {
                        capability: "message_context:full".to_string(),
                        reason: "full context sharing requires explicit opt-in".to_string(),
                    });
                }
                sanitized.messages = context.messages.clone();
            }
        }

        Ok(sanitized)
    }
}

fn sanitize_refs(
    resource: &str,
    policy: &ResourceSharingPolicy,
    refs: &[String],
) -> Result<Vec<String>, SubAgentError> {
    match policy {
        ResourceSharingPolicy::None => Ok(Vec::new()),
        ResourceSharingPolicy::ReadOnly | ResourceSharingPolicy::Inherited { explicit: true } => {
            Ok(refs.to_vec())
        }
        ResourceSharingPolicy::Scoped { refs: allowed } => Ok(refs
            .iter()
            .filter(|reference| allowed.contains(*reference))
            .cloned()
            .collect()),
        ResourceSharingPolicy::Inherited { explicit: false } => {
            Err(SubAgentError::PermissionDenied {
                capability: resource.to_string(),
                reason: "inherited context sharing requires explicit opt-in".to_string(),
            })
        }
    }
}

fn summary_msg(text: &str) -> Msg {
    Msg::new(
        "context_summary".to_string(),
        vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
        Role::System,
    )
    .expect("system text message is valid")
}
