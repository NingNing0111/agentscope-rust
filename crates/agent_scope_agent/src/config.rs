//! Configuration types for agent construction and behavior.
//!
//! Includes [`AgentConfig`] (construction parameters), [`ReActConfig`] (loop behavior),
//! and [`ContextConfig`] (context window management).

use std::sync::Arc;

use agent_scope_model::ChatModel;
use agent_scope_state::SessionStore;
use agent_scope_tool::ToolKit;

use crate::agent_error::AgentError;
use crate::permission::{PermissionContext, PermissionMode};

// ---------------------------------------------------------------------------
// AgentConfig
// ---------------------------------------------------------------------------

/// Construction configuration for an agent.
///
/// Set once at construction time; immutable thereafter.
pub struct AgentConfig {
    /// Agent identifier — used in messages and events.
    pub name: String,
    /// System prompt prepended to model context.
    pub system_prompt: String,
    /// Model for reasoning calls.
    pub model: Arc<dyn ChatModel>,
    /// Registered tools for tool-calling (optional).
    pub toolkit: Option<ToolKit>,
    /// Streaming channel capacity: `None` = unbounded (default), `Some(N)` = bounded.
    pub stream_channel_capacity: Option<usize>,
    /// Permission context used to authorize tool execution.
    pub permission_context: PermissionContext,
    /// Whether the built-in task planning tools (TaskCreate/TaskList/TaskGet/
    /// TaskUpdate) are registered at construction time, and whether the
    /// unfinished-task reminder injection is active. Default: `true`.
    pub task_tools_enabled: bool,
    /// Session storage backend for agent-state persistence. When `None`, the
    /// default [`JsonFileSessionStore`](agent_scope_state::JsonFileSessionStore)
    /// rooted at `sessions/` is used at construction time (Feature 025).
    pub session_store: Option<Arc<dyn SessionStore>>,
    /// Session identifier. When set, the agent is built from any state already
    /// persisted under this id; when unset, a fresh session id is generated.
    pub session_id: Option<String>,
    /// Whether the agent automatically persists its latest state after each
    /// reply ends (including interruption/cancellation). Default: `true`.
    /// When `false`, no storage writes occur at all.
    pub auto_persist: bool,
}

impl AgentConfig {
    /// Create a new builder.
    pub fn builder() -> AgentConfigBuilder {
        AgentConfigBuilder::default()
    }
}

/// Builder for [`AgentConfig`].
pub struct AgentConfigBuilder {
    name: Option<String>,
    system_prompt: String,
    model: Option<Arc<dyn ChatModel>>,
    toolkit: Option<ToolKit>,
    stream_channel_capacity: Option<usize>,
    permission_context: PermissionContext,
    task_tools_enabled: bool,
    session_store: Option<Arc<dyn SessionStore>>,
    session_id: Option<String>,
    auto_persist: bool,
}

impl Default for AgentConfigBuilder {
    fn default() -> Self {
        Self {
            name: None,
            system_prompt: String::new(),
            model: None,
            toolkit: None,
            stream_channel_capacity: None,
            permission_context: PermissionContext::default(),
            task_tools_enabled: true,
            session_store: None,
            session_id: None,
            auto_persist: true,
        }
    }
}

impl AgentConfigBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    pub fn model(mut self, model: Arc<dyn ChatModel>) -> Self {
        self.model = Some(model);
        self
    }

    pub fn toolkit(mut self, toolkit: ToolKit) -> Self {
        self.toolkit = Some(toolkit);
        self
    }

    /// Set the permission context used to authorize tool execution.
    pub fn permission_context(mut self, context: PermissionContext) -> Self {
        self.permission_context = context;
        self
    }

    /// Set the permission mode while keeping existing rules.
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_context.mode = mode;
        self
    }

    /// Enable or disable the built-in task planning tools
    /// (TaskCreate/TaskList/TaskGet/TaskUpdate) and the unfinished-task
    /// reminder injection. Enabled by default.
    pub fn task_tools_enabled(mut self, enabled: bool) -> Self {
        self.task_tools_enabled = enabled;
        self
    }

    /// Inject a session storage backend for agent-state persistence.
    ///
    /// When not set, a default `JsonFileSessionStore` rooted at `sessions/` is
    /// created at agent construction time (Feature 025).
    pub fn session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(store);
        self
    }

    /// Set the session identifier used to persist / resume agent state.
    ///
    /// When set, the agent is built from any state already persisted under
    /// this id; when unset, a fresh session id is generated.
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Toggle automatic persistence of agent state after each reply.
    ///
    /// Defaults to `true`. When `false`, no storage writes occur at all
    /// (spec FR-007 / SC-007).
    pub fn auto_persist(mut self, enabled: bool) -> Self {
        self.auto_persist = enabled;
        self
    }

    /// Set the streaming channel capacity.
    ///
    /// - `None` = unbounded channel (default, per FR-019)
    /// - `Some(n)` = bounded channel with capacity `n`
    ///
    /// **Panics**: if `Some(0)` is passed (capacity must be > 0).
    pub fn with_stream_channel_capacity(mut self, cap: Option<usize>) -> Self {
        if let Some(0) = cap {
            panic!("stream_channel_capacity: `Some(0)` is invalid; use `None` for unbounded");
        }
        self.stream_channel_capacity = cap;
        self
    }

    /// Build the config, validating all fields.
    pub fn build(self) -> Result<AgentConfig, AgentError> {
        let name = self.name.ok_or_else(|| AgentError::InvalidConfig {
            field: "name".into(),
            message: "name is required".into(),
        })?;

        if name.is_empty() {
            return Err(AgentError::InvalidConfig {
                field: "name".into(),
                message: "name must not be empty".into(),
            });
        }

        let model = self.model.ok_or_else(|| AgentError::InvalidConfig {
            field: "model".into(),
            message: "model is required".into(),
        })?;

        Ok(AgentConfig {
            name,
            system_prompt: self.system_prompt,
            model,
            toolkit: self.toolkit,
            stream_channel_capacity: self.stream_channel_capacity,
            permission_context: self.permission_context,
            task_tools_enabled: self.task_tools_enabled,
            session_store: self.session_store,
            session_id: self.session_id,
            auto_persist: self.auto_persist,
        })
    }
}

// ---------------------------------------------------------------------------
// ReActConfig
// ---------------------------------------------------------------------------

/// Loop behavior configuration for [`ReActAgent`](super::ReActAgent).
#[derive(Debug, Clone)]
pub struct ReActConfig {
    /// Maximum reasoning-acting iterations per reply.
    pub max_iters: u32,
    /// Stop on permission denial (vs. waiting for confirmation).
    pub stop_on_reject: bool,
    /// Message returned when the reply is interrupted.
    pub interruption_message: String,
    /// Extra iterations allowed when structured output parsing fails.
    pub structured_output_grace_iters: u32,
}

impl Default for ReActConfig {
    fn default() -> Self {
        Self {
            max_iters: 20,
            stop_on_reject: false,
            interruption_message: "The execution was interrupted.".into(),
            structured_output_grace_iters: 3,
        }
    }
}

impl ReActConfig {
    /// Validate configuration.
    pub fn validate(&self) -> Result<(), AgentError> {
        if self.max_iters == 0 {
            return Err(AgentError::InvalidConfig {
                field: "max_iters".into(),
                message: "max_iters must be > 0".into(),
            });
        }
        if self.structured_output_grace_iters == 0 {
            return Err(AgentError::InvalidConfig {
                field: "structured_output_grace_iters".into(),
                message: "structured_output_grace_iters must be > 0".into(),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ContextConfig
// ---------------------------------------------------------------------------

/// Context window management configuration.
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Whether context compression is enabled (default: false).
    /// When enabled, compression runs each iteration before model.call().
    /// Corresponds to Python `ContextConfig.enable` in `_react_agent.py`.
    pub enable: bool,
    /// Fraction of context_size that triggers compression (0 < ratio < 1.0).
    pub trigger_ratio: f64,
    /// Fraction of context_size reserved for model response (0 <= ratio < trigger_ratio).
    pub reserve_ratio: f64,
    /// System prompt for compression model calls.
    pub compression_prompt: String,
    /// Truncation limit for tool result content (characters).
    pub tool_result_limit: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            enable: false,
            trigger_ratio: 0.8,
            reserve_ratio: 0.1,
            compression_prompt: "<STD_CP_PROMPT>".into(),
            tool_result_limit: 4096,
        }
    }
}

impl ContextConfig {
    /// Validate configuration.
    pub fn validate(&self) -> Result<(), AgentError> {
        if self.trigger_ratio <= 0.0 || self.trigger_ratio >= 1.0 {
            return Err(AgentError::InvalidConfig {
                field: "trigger_ratio".into(),
                message: "trigger_ratio must be in (0.0, 1.0)".into(),
            });
        }
        if self.reserve_ratio < 0.0 || self.reserve_ratio >= self.trigger_ratio {
            return Err(AgentError::InvalidConfig {
                field: "reserve_ratio".into(),
                message: "reserve_ratio must be in [0.0, trigger_ratio)".into(),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::Msg;
    use agent_scope_model::{ChatResponse, ModelCallResult, ModelError};
    use serde_json::Value as JsonValue;

    struct DummyModel {
        name: String,
    }

    #[async_trait::async_trait]
    impl ChatModel for DummyModel {
        fn model_name(&self) -> &str {
            &self.name
        }
        fn stream_enabled(&self) -> bool {
            false
        }
        async fn call_api(
            &self,
            _model: &str,
            _messages: &[Msg],
            _tools: Option<&[JsonValue]>,
            _tool_choice: Option<&agent_scope_model::ToolChoice>,
        ) -> Result<ModelCallResult, ModelError> {
            Ok(ModelCallResult::Complete(ChatResponse::default()))
        }
    }

    /// T014: Empty name rejected.
    #[test]
    fn test_agent_config_empty_name_rejected() {
        let result = AgentConfig::builder().name("").build();
        assert!(result.is_err());
        if let Err(AgentError::InvalidConfig { field, .. }) = result {
            assert_eq!(field, "name");
        } else {
            panic!("expected InvalidConfig");
        }
    }

    /// T014: Missing name rejected.
    #[test]
    fn test_agent_config_missing_name_rejected() {
        let result = AgentConfig::builder().build();
        assert!(result.is_err());
    }

    /// T014: Missing model rejected.
    #[test]
    fn test_agent_config_missing_model_rejected() {
        let result = AgentConfig::builder().name("test").build();
        assert!(matches!(result, Err(AgentError::InvalidConfig { .. })));
    }

    /// T014: Valid config accepted.
    #[test]
    fn test_agent_config_valid_accepted() {
        let model = Arc::new(DummyModel {
            name: "dummy".into(),
        });
        let config = AgentConfig::builder()
            .name("test-agent")
            .model(model)
            .build()
            .unwrap();
        assert_eq!(config.name, "test-agent");
        assert!(config.system_prompt.is_empty());
        assert!(config.toolkit.is_none());
        assert!(config.task_tools_enabled);
    }

    /// T024: task_tools_enabled defaults to true; explicit disable works.
    #[test]
    fn test_agent_config_task_tools_toggle() {
        let model = || {
            Arc::new(DummyModel {
                name: "dummy".into(),
            }) as Arc<dyn ChatModel>
        };
        let enabled = AgentConfig::builder()
            .name("a")
            .model(model())
            .build()
            .unwrap();
        assert!(enabled.task_tools_enabled);

        let disabled = AgentConfig::builder()
            .name("a")
            .model(model())
            .task_tools_enabled(false)
            .build()
            .unwrap();
        assert!(!disabled.task_tools_enabled);
    }

    /// T015: ReActConfig default values.
    #[test]
    fn test_react_config_defaults() {
        let config = ReActConfig::default();
        assert_eq!(config.max_iters, 20);
        assert!(!config.stop_on_reject);
        assert_eq!(config.structured_output_grace_iters, 3);
        assert!(!config.interruption_message.is_empty());
    }

    /// T015: ReActConfig max_iters=0 rejected.
    #[test]
    fn test_react_config_max_iters_zero_rejected() {
        let config = ReActConfig {
            max_iters: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    /// T015: ReActConfig valid config passes.
    #[test]
    fn test_react_config_valid_passes() {
        let config = ReActConfig::default();
        assert!(config.validate().is_ok());
    }

    /// ContextConfig defaults.
    #[test]
    fn test_context_config_defaults() {
        let config = ContextConfig::default();
        assert!(!config.enable);
        assert_eq!(config.trigger_ratio, 0.8);
        assert_eq!(config.reserve_ratio, 0.1);
        assert_eq!(config.tool_result_limit, 4096);
    }

    /// ContextConfig invalid trigger_ratio rejected.
    #[test]
    fn test_context_config_invalid_trigger_ratio() {
        assert!(
            ContextConfig {
                trigger_ratio: 0.0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            ContextConfig {
                trigger_ratio: 1.0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    /// ContextConfig reserve >= trigger rejected.
    #[test]
    fn test_context_config_reserve_ge_trigger_rejected() {
        assert!(
            ContextConfig {
                trigger_ratio: 0.5,
                reserve_ratio: 0.5,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
