//! Configuration types for agent construction and behavior.
//!
//! Includes [`AgentConfig`] (construction parameters), [`ReActConfig`] (loop behavior),
//! and [`ContextConfig`] (context window management).

use std::collections::HashMap;
use std::sync::Arc;

use agent_scope_model::ChatModel;
use agent_scope_state::SessionStore;
use agent_scope_tool::ToolKit;
use agent_scope_workspace::WorkspaceBase;

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
    /// Runtime-state injection configuration (Feature 026). Defaults to
    /// [`InjectionConfig::default()`] (injection enabled).
    pub injection_config: InjectionConfig,
    /// Optional workspace bound to this agent (Feature 029).
    ///
    /// When set, the agent construction path automatically merges legacy
    /// workspace built-in tools (`Bash`/`Read`/`Edit`/`Write`/`Grep`/`Glob`/
    /// `ResetTools`/`Skill`) plus pi-compatible lowercase tools (`bash`/`read`/
    /// `edit`/`write`/`grep`/`find`/`ls`) into the agent's `ToolKit`. Windows
    /// additionally gets `PowerShell` and `powershell`. Agents without a
    /// workspace expose no file/command tools.
    pub workspace: Option<Arc<dyn WorkspaceBase>>,
    /// Whether workspace built-in tools are injected when a workspace is
    /// present. Defaults to `true`.
    pub workspace_tools_enabled: bool,
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
    injection_config: InjectionConfig,
    workspace: Option<Arc<dyn WorkspaceBase>>,
    workspace_tools_enabled: bool,
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
            injection_config: InjectionConfig::default(),
            workspace: None,
            workspace_tools_enabled: true,
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

    /// Set the runtime-state injection configuration (Feature 026).
    ///
    /// When not set, an `InjectionConfig::default()` is used (injection
    /// enabled). Validated at build time.
    pub fn injection_config(mut self, config: InjectionConfig) -> Self {
        self.injection_config = config;
        self
    }

    /// Bind a workspace to this agent (Feature 029).
    ///
    /// When set, the workspace built-in tools are automatically injected into
    /// the agent's `ToolKit` at construction time.
    pub fn workspace(mut self, workspace: Arc<dyn WorkspaceBase>) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Enable or disable automatic injection of workspace built-in tools.
    /// Enabled by default when a workspace is bound.
    pub fn workspace_tools_enabled(mut self, enabled: bool) -> Self {
        self.workspace_tools_enabled = enabled;
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

        self.injection_config.validate()?;

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
            injection_config: self.injection_config,
            workspace: self.workspace,
            workspace_tools_enabled: self.workspace_tools_enabled,
        })
    }
}

// ---------------------------------------------------------------------------
// InjectionConfig
// ---------------------------------------------------------------------------

/// Default template wrapping injected runtime-state fields.
///
/// Aligns with Python `InjectionConfig.template`. The `{runtime_state}`
/// placeholder is replaced with the joined `<...>` fields.
pub const DEFAULT_INJECTION_TEMPLATE: &str = "<system-reminder>Treat the following as the ground truth at this point of the conversation. Anything stated earlier is outdated, and a later reminder, if any, supersedes this one:\n{runtime_state}\n</system-reminder>";

/// Default source identifier marking the agent's own runtime-state injections.
///
/// Aligns with Python `InjectionConfig.injection_source`. Used to detect
/// existing injections when scanning the context.
pub const DEFAULT_INJECTION_SOURCE: &str = r#"{"label": "System", "sublabel": "Runtime State"}"#;

/// Default names of the task-related tools whose presence in the context
/// suppresses the tasks injection. Aligns with Python
/// `InjectionConfig.task_tool_names`.
pub const DEFAULT_TASK_TOOL_NAMES: [&str; 4] = ["TaskCreate", "TaskGet", "TaskList", "TaskUpdate"];

/// Runtime-state injection configuration (Feature 026).
///
/// Controls how the agent injects time, unfinished-task and context-length
/// information into the conversation context each iteration. Fields, defaults
/// and semantics align with Python `InjectionConfig` (upstream `9d1026fa`).
#[derive(Debug, Clone)]
pub struct InjectionConfig {
    /// Master switch. When `false`, no dimension is evaluated or injected and
    /// no hint event is emitted (FR-011).
    pub inject_runtime_state: bool,
    /// IANA timezone name used to compute and format the injected time.
    /// Defaults to `"UTC"`. Unresolvable names fall back to UTC at runtime.
    pub timezone: String,
    /// strftime-style format for injected/parsed times. Must round-trip a full
    /// timestamp (carry the date part). Defaults to `%Y-%m-%dT%H:%M:%S`.
    pub time_format: String,
    /// Minimum elapsed time (hours) since the last recorded injection before a
    /// new time injection is triggered.
    pub time_interval: f64,
    /// Buffer ahead of the compression threshold that activates the
    /// context-length injection, in `[0, 1]` and smaller than
    /// `ContextConfig.trigger_ratio`.
    pub context_buffer_ratio: f64,
    /// Wrapping template containing the `{runtime_state}` placeholder.
    pub template: String,
    /// Fixed source used to identify the agent's own injections in the context.
    pub injection_source: String,
    /// Names of task-related tools whose tool calls mark the agent as aware of
    /// the tasks, suppressing the tasks injection.
    pub task_tool_names: Vec<String>,
    /// User-defined fields attached to every triggered injection; they never
    /// trigger an injection by themselves.
    pub extra_fields: HashMap<String, String>,
    /// Whether a `HintBlockEvent` is emitted when an injection happens.
    pub emit_hint_event: bool,
}

impl Default for InjectionConfig {
    fn default() -> Self {
        Self {
            inject_runtime_state: true,
            timezone: "UTC".into(),
            time_format: "%Y-%m-%dT%H:%M:%S".into(),
            time_interval: 0.5,
            context_buffer_ratio: 0.2,
            template: DEFAULT_INJECTION_TEMPLATE.into(),
            injection_source: DEFAULT_INJECTION_SOURCE.into(),
            task_tool_names: DEFAULT_TASK_TOOL_NAMES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            extra_fields: HashMap::new(),
            emit_hint_event: true,
        }
    }
}

impl InjectionConfig {
    /// Validate the configuration (FR-007 / FR-014).
    ///
    /// Rejects: a template missing the `{runtime_state}` placeholder; a
    /// `time_format` that cannot round-trip a full timestamp; a negative
    /// `time_interval`; a `context_buffer_ratio` outside `[0, 1]`.
    /// An unresolvable `timezone` is **not** rejected — it falls back to UTC
    /// at runtime (aligned with Python `_resolve_timezone`).
    ///
    /// The `context_buffer_ratio < trigger_ratio` cross-field check is done by
    /// [`Self::validate_with_trigger`], which has access to the real context
    /// compression trigger ratio.
    pub fn validate(&self) -> Result<(), AgentError> {
        if !self.template.contains("{runtime_state}") {
            return Err(AgentError::InvalidConfig {
                field: "injection_config.template".into(),
                message: "the injection template must contain the '{runtime_state}' placeholder"
                    .into(),
            });
        }
        if !time_format_round_trips(&self.time_format) {
            return Err(AgentError::InvalidConfig {
                field: "injection_config.time_format".into(),
                message: "time_format must round-trip a full timestamp (carry the date part)"
                    .into(),
            });
        }
        if self.time_interval < 0.0 {
            return Err(AgentError::InvalidConfig {
                field: "injection_config.time_interval".into(),
                message: "time_interval must be >= 0".into(),
            });
        }
        if !(0.0..=1.0).contains(&self.context_buffer_ratio) {
            return Err(AgentError::InvalidConfig {
                field: "injection_config.context_buffer_ratio".into(),
                message: "context_buffer_ratio must be in [0, 1]".into(),
            });
        }
        for key in self.extra_fields.keys() {
            if !is_valid_extra_field_key(key) {
                return Err(AgentError::InvalidConfig {
                    field: "injection_config.extra_fields".into(),
                    message: format!(
                        "extra_fields key '{key}' must be non-empty ASCII [A-Za-z0-9_-] and must not be a reserved runtime-state key"
                    ),
                });
            }
        }
        Ok(())
    }

    /// Validate against the real context compression trigger ratio.
    ///
    /// Called by [`ReActAgent::new`](crate::ReActAgent) once the
    /// `ContextConfig` is available, in addition to [`Self::validate`].
    pub fn validate_with_trigger(&self, context_trigger_ratio: f64) -> Result<(), AgentError> {
        self.validate()?;
        if self.context_buffer_ratio >= context_trigger_ratio {
            return Err(AgentError::InvalidConfig {
                field: "injection_config.context_buffer_ratio".into(),
                message: "context_buffer_ratio must be smaller than the context compression trigger_ratio".into(),
            });
        }
        Ok(())
    }
}

/// Whether `time_format` can round-trip a full timestamp (format → parse back
/// to a time carrying the date part). A time-only format such as `%H:%M:%S`
/// fails because the parsed time falls back to year 1900.
fn is_valid_extra_field_key(key: &str) -> bool {
    const RESERVED: [&str; 4] = ["current-time", "timezone", "tasks", "context-length"];
    !key.is_empty()
        && !RESERVED.contains(&key)
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn time_format_round_trips(time_format: &str) -> bool {
    use chrono::{NaiveDate, NaiveDateTime};
    // Pick a fixed instant that exercises date + time fields.
    let instant = NaiveDate::from_ymd_opt(2026, 7, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let formatted = instant.format(time_format).to_string();
    // Parse back; NaiveDateTime requires the format to carry a date part.
    NaiveDateTime::parse_from_str(&formatted, time_format).is_ok()
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
    fn test_injection_config_rejects_invalid_extra_field_keys() {
        for key in ["", "tasks", "current-time", "bad key", "bad<tag>", "ключ"] {
            let mut config = InjectionConfig::default();
            config.extra_fields.insert(key.into(), "value".into());
            assert!(
                config.validate().is_err(),
                "key should be rejected: {key:?}"
            );
        }

        let mut config = InjectionConfig::default();
        config
            .extra_fields
            .insert("valid_key-1".into(), "value".into());
        assert!(config.validate().is_ok());
    }

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

    /// T026: InjectionConfig default values align with Python.
    #[test]
    fn test_injection_config_defaults() {
        let config = InjectionConfig::default();
        assert!(config.inject_runtime_state);
        assert_eq!(config.timezone, "UTC");
        assert_eq!(config.time_format, "%Y-%m-%dT%H:%M:%S");
        assert_eq!(config.time_interval, 0.5);
        assert_eq!(config.context_buffer_ratio, 0.2);
        assert!(config.template.contains("{runtime_state}"));
        assert_eq!(
            config.injection_source,
            r#"{"label": "System", "sublabel": "Runtime State"}"#
        );
        assert_eq!(config.task_tool_names.len(), 4);
        assert!(config.task_tool_names.contains(&"TaskCreate".to_string()));
        assert!(config.task_tool_names.contains(&"TaskUpdate".to_string()));
        assert!(config.extra_fields.is_empty());
        assert!(config.emit_hint_event);
    }

    /// T026: Template missing placeholder rejected.
    #[test]
    fn test_injection_config_template_missing_placeholder_rejected() {
        let config = InjectionConfig {
            template: "<system-reminder></system-reminder>".into(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
        if let Err(AgentError::InvalidConfig { field, .. }) = config.validate() {
            assert_eq!(field, "injection_config.template");
        } else {
            panic!("expected InvalidConfig");
        }
    }

    /// T026: Time-only format (no date part) rejected.
    #[test]
    fn test_injection_config_time_only_format_rejected() {
        let config = InjectionConfig {
            time_format: "%H:%M:%S".into(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    /// T026: Negative time_interval rejected.
    #[test]
    fn test_injection_config_negative_interval_rejected() {
        let config = InjectionConfig {
            time_interval: -0.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    /// T026: context_buffer_ratio out of [0,1] rejected.
    #[test]
    fn test_injection_config_buffer_ratio_out_of_range_rejected() {
        for bad in [1.5, -0.1] {
            let config = InjectionConfig {
                context_buffer_ratio: bad,
                ..Default::default()
            };
            assert!(config.validate().is_err(), "ratio {bad} should be rejected");
        }
    }

    /// T026: context_buffer_ratio >= trigger_ratio rejected via
    /// validate_with_trigger.
    #[test]
    fn test_injection_config_buffer_ratio_ge_trigger_rejected() {
        let config = InjectionConfig {
            context_buffer_ratio: 0.8,
            ..Default::default()
        };
        assert!(config.validate().is_ok()); // standalone validation passes
        assert!(config.validate_with_trigger(0.8).is_err());
    }

    /// T026: Invalid timezone NOT rejected (falls back to UTC at runtime).
    #[test]
    fn test_injection_config_invalid_timezone_not_rejected() {
        let config = InjectionConfig {
            timezone: "Mars/Olympus_Mons".into(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    /// T026: InjectionConfig attached via builder and validated at build time.
    #[test]
    fn test_agent_config_injection_config_builder() {
        let model = || {
            Arc::new(DummyModel {
                name: "dummy".into(),
            }) as Arc<dyn ChatModel>
        };
        let config = AgentConfig::builder()
            .name("a")
            .model(model())
            .injection_config(InjectionConfig::default())
            .build()
            .unwrap();
        assert!(config.injection_config.inject_runtime_state);

        // A config with an invalid template fails at build.
        let result = AgentConfig::builder()
            .name("a")
            .model(model())
            .injection_config(InjectionConfig {
                template: "<system-reminder></system-reminder>".into(),
                ..Default::default()
            })
            .build();
        assert!(result.is_err());
    }
}
