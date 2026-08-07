//! Tests for the unified runtime-state injection pipeline (Feature 026).
//!
//! Mirrors the Python `tests/agent_injection_test.py` behavior cases:
//! time injection, task reminder injection, context-length injection, extra
//! fields, master switch, template validation and timezone fallback. The
//! pipeline takes an explicit `now` so the wall-clock time is frozen across
//! tests (aligning with Python's `_FrozenDatetime` patch).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use agent_scope_agent::config::{DEFAULT_INJECTION_SOURCE, InjectionConfig};
use agent_scope_agent::runtime_injection::maybe_inject_runtime_state;
use agent_scope_message::{ContentBlock, HintBlock, HintContent, Msg, Role, ToolCallBlock};
use agent_scope_state::{AgentState, Task, TaskState};
use chrono::{DateTime, FixedOffset};

/// The fixed "now" used across the tests, so time-related assertions are
/// deterministic. 2026-07-01 12:00:00 UTC.
const FROZEN_NOW_UTC: &str = "2026-07-01T12:00:00Z";

fn frozen_now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(FROZEN_NOW_UTC).unwrap()
}

fn default_config() -> InjectionConfig {
    InjectionConfig::default()
}

/// Build an agent state with the given context messages.
fn state_with_context(context: Vec<Msg>) -> Arc<RwLock<AgentState>> {
    let mut s = AgentState::new();
    s.context = context;
    Arc::new(RwLock::new(s))
}

fn assistant_msg(blocks: Vec<ContentBlock>) -> Msg {
    Msg::new("assistant".into(), blocks, Role::Assistant).unwrap()
}

fn plain_user_msg(text: &str) -> Msg {
    Msg::new(
        "user".into(),
        vec![ContentBlock::Text(agent_scope_message::TextBlock::new(
            text.into(),
        ))],
        Role::User,
    )
    .unwrap()
}

/// Append an existing runtime-state injection carrying `time_str`, which is
/// the wall-clock time of `timezone` (mirrors Python `_add_injection`).
fn add_injection(state: &RwLock<AgentState>, time_str: &str, timezone: &str) {
    let hint = format!("<current-time>{time_str}</current-time>\n<timezone>{timezone}</timezone>");
    let block = HintBlock {
        hint: HintContent::Text(hint),
        source: Some(DEFAULT_INJECTION_SOURCE.to_string()),
        id: "h-1".into(),
        created_at: String::new(),
        finished_at: None,
    };
    let mut state = state.write().unwrap();
    state
        .context
        .push(assistant_msg(vec![ContentBlock::Hint(block)]));
}

fn task(id: &str, state: TaskState) -> Task {
    let mut t = Task::new(format!("task {id}"), "desc".into(), HashMap::new());
    t.id = id.to_string();
    t.state = state;
    t
}

/// Get the injected hint text (if any) appended to the context tail.
fn last_hint(state: &RwLock<AgentState>) -> Option<String> {
    let ctx = state.read().unwrap().context.clone();
    ctx.last().and_then(|m| {
        m.content.iter().find_map(|b| match b {
            ContentBlock::Hint(h) => match &h.hint {
                HintContent::Text(t) => Some(t.clone()),
                HintContent::Blocks(_) => None,
            },
            _ => None,
        })
    })
}

/// Run the pipeline with default config and collect the event.
fn run(
    state: &RwLock<AgentState>,
    config: &InjectionConfig,
    cur_iter: u32,
    input_tokens: Option<usize>,
    task_tools_enabled: bool,
) -> Option<agent_scope_event::HintBlockEvent> {
    maybe_inject_runtime_state(
        state,
        "assistant",
        config,
        frozen_now(),
        cur_iter,
        input_tokens,
        1000,
        0.8,
        task_tools_enabled,
    )
}

// ===========================================================================
// US1: Time injection
// ===========================================================================

#[test]
fn test_first_reply_triggers_time_injection() {
    let state = state_with_context(vec![plain_user_msg("hello")]);
    let evt = run(&state, &default_config(), 1, Some(0), true).expect("injection");
    assert_eq!(evt.source.as_deref(), Some(DEFAULT_INJECTION_SOURCE));
    let hint = evt.hint;
    let text = match &hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text hint"),
    };
    assert!(
        text.contains("<current-time>2026-07-01T12:00:00</current-time>"),
        "got: {text}"
    );
    assert!(text.contains("<timezone>UTC</timezone>"), "got: {text}");
    assert!(text.starts_with("<system-reminder>"), "got: {text}");
    assert!(text.ends_with("</system-reminder>"), "got: {text}");

    // The same hint block was appended to the context.
    let appended = last_hint(&state).expect("hint appended");
    assert_eq!(appended, text);
}

#[test]
fn test_long_interval_triggers_reinject_recent_skips() {
    // 6 hours ago → re-inject.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    add_injection(&state, "2026-07-01T06:00:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(evt.is_some(), "6h ago should re-inject");

    // 10 minutes ago (< time_interval 0.5h) → skip.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    add_injection(&state, "2026-07-01T11:50:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(evt.is_none(), "10m ago should not re-inject");
}

#[test]
fn test_injection_after_compression() {
    // A recent injection before "compression" → no new injection.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    add_injection(&state, "2026-07-01T12:00:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(evt.is_none(), "recent injection should suppress");

    // Simulate compression dropping the old context → inject again.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(evt.is_some(), "after compression should inject");
}

/// Regression: the injected time must be the wall-clock of the configured
/// timezone (not UTC), and a just-injected time must not re-inject every
/// iteration (previously the injected text carried the UTC time while the
/// `<timezone>` tag named the configured zone, so the elapsed-time calculation
/// drifted by the UTC offset and re-injected on every iteration).
#[test]
fn test_non_utc_timezone_injects_local_time_and_does_not_reinject() {
    let config = InjectionConfig {
        timezone: "Asia/Shanghai".into(),
        ..Default::default()
    };
    let state = state_with_context(vec![plain_user_msg("hello")]);
    let evt = run(&state, &config, 1, Some(0), true).expect("injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    // Frozen now is 2026-07-01T12:00:00Z == 20:00 in Shanghai.
    assert!(
        text.contains("<current-time>2026-07-01T20:00:00</current-time>"),
        "injected time must be the Shanghai wall-clock: {text}"
    );
    assert!(text.contains("<timezone>Asia/Shanghai</timezone>"));

    // A just-injected time at the same instant must suppress a re-injection.
    let evt2 = run(&state, &config, 2, None, true);
    assert!(
        evt2.is_none(),
        "non-UTC timezone must not re-inject immediately"
    );
}

#[test]
fn test_recorded_timezone_is_honored() {
    // Frozen now is 12:00 UTC = 20:00 Shanghai. An injection recorded 10
    // minutes ago in Shanghai → within interval, skip.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    add_injection(&state, "2026-07-01T19:50:00", "Asia/Shanghai");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(evt.is_none(), "Shanghai timezone should be honored");

    // The same wall-clock time read as UTC would be ~7h50m in the future, so a
    // negative elapsed time must trigger an injection instead of being skipped.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    add_injection(&state, "2026-07-01T19:50:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(
        evt.is_some(),
        "negative elapsed (clock went backwards) should inject"
    );
}

#[test]
fn test_invalid_timezone_falls_back_to_utc() {
    // Unresolvable timezone shouldn't break the pipeline.
    let config = InjectionConfig {
        timezone: "Mars/Olympus_Mons".into(),
        ..Default::default()
    };
    let state = state_with_context(vec![plain_user_msg("hello")]);
    let evt = run(&state, &config, 1, Some(0), true).expect("injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    // The wall-clock is computed in UTC (fallback), but the injected timezone
    // text keeps the raw configured value (Python behavior).
    assert!(
        text.contains("<current-time>2026-07-01T12:00:00</current-time>"),
        "got: {text}"
    );
    assert!(
        text.contains("<timezone>Mars/Olympus_Mons</timezone>"),
        "got: {text}"
    );
}

// ===========================================================================
// US2: Task reminder injection
// ===========================================================================

#[test]
fn test_pending_task_triggers_tasks_injection() {
    let mut s = AgentState::new();
    s.tasks_context.add_task(task("1", TaskState::Pending));
    s.context = vec![plain_user_msg("hi")];
    let state = Arc::new(RwLock::new(s));

    let evt = run(&state, &default_config(), 2, None, true).expect("tasks injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    assert!(
        text.contains("<tasks>You have 0 in-progress tasks and 1 pending tasks. Use `TaskList` to view them if you don't know.</tasks>"),
        "got: {text}"
    );

    // The just-injected reminder makes the context aware → no repeated injection.
    let evt2 = run(&state, &default_config(), 3, None, true);
    assert!(evt2.is_none(), "should not repeat the tasks reminder");
}

#[test]
fn test_aware_by_tool_call_suppresses_tasks() {
    let mut s = AgentState::new();
    s.tasks_context.add_task(task("1", TaskState::Pending));
    let tc = ToolCallBlock::new("c1".into(), "TaskCreate".into(), "{}".into());
    s.context = vec![
        plain_user_msg("hi"),
        assistant_msg(vec![ContentBlock::ToolCall(tc)]),
    ];
    let state = Arc::new(RwLock::new(s));

    // cur_iter=2 skips context-length; no time injection (no recorded time →
    // time WOULD inject, but with a recent injection it doesn't). Here the
    // context has a tool call making the agent aware of tasks, so the tasks
    // dimension is suppressed. With no `<current-time>` injection, the time
    // dimension would inject — to isolate the tasks behavior, seed a recent
    // time injection so the time dimension is suppressed too.
    add_injection(&state, "2026-07-01T12:00:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(
        evt.is_none(),
        "aware by tool call should suppress tasks injection"
    );
}

#[test]
fn test_aware_by_previous_reminder_suppresses_tasks() {
    let mut s = AgentState::new();
    s.tasks_context.add_task(task("1", TaskState::Pending));
    let reminder = assistant_msg(vec![ContentBlock::Hint(HintBlock {
        hint: HintContent::Text(
            "<tasks>You have 0 in-progress tasks and 1 pending tasks.</tasks>".into(),
        ),
        source: Some(DEFAULT_INJECTION_SOURCE.to_string()),
        id: "h-1".into(),
        created_at: String::new(),
        finished_at: None,
    })]);
    s.context = vec![plain_user_msg("hi"), reminder];
    let state = Arc::new(RwLock::new(s));

    add_injection(&state, "2026-07-01T12:00:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(evt.is_none(), "aware by previous reminder should suppress");
}

#[test]
fn test_task_tools_disabled_suppresses_tasks_dimension() {
    let mut s = AgentState::new();
    s.tasks_context.add_task(task("1", TaskState::Pending));
    s.context = vec![plain_user_msg("hi")];
    let state = Arc::new(RwLock::new(s));

    // task_tools_enabled=false suppresses the tasks dimension. With a recent
    // time injection seeded, neither dimension fires → zero injection.
    add_injection(&state, "2026-07-01T12:00:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, false);
    assert!(
        evt.is_none(),
        "task_tools_disabled should suppress tasks dimension"
    );
}

#[test]
fn test_master_switch_disables_all_dimensions() {
    let mut s = AgentState::new();
    s.tasks_context.add_task(task("1", TaskState::Pending));
    s.context = vec![plain_user_msg("hi")];
    let state = Arc::new(RwLock::new(s));

    let config = InjectionConfig {
        inject_runtime_state: false,
        ..Default::default()
    };
    let evt = run(&state, &config, 1, Some(700), true);
    assert!(evt.is_none(), "master switch off → zero injection");
    assert_eq!(state.read().unwrap().context.len(), 1, "no context append");
}

// ===========================================================================
// US3: Context-length injection
// ===========================================================================

#[test]
fn test_context_size_triggers_injection() {
    // First iteration + 700 tokens: 700 > (0.8 - 0.2) * 1000 == 600 → triggers.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let evt = run(&state, &default_config(), 1, Some(700), true).expect("context injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    assert!(
        text.contains("<context-length>Your current context contains 700 tokens. When reaching 800 tokens, your context will be compressed.</context-length>"),
        "got: {text}"
    );
}

#[test]
fn test_context_size_not_triggered_below_threshold() {
    // Non-first-iteration (cur_iter=2) skips the context-length dimension
    // entirely, even with a high token count. A recent time injection is
    // seeded so the time dimension is suppressed too → zero injection.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    add_injection(&state, "2026-07-01T12:00:00", "UTC");
    let evt = run(&state, &default_config(), 2, Some(700), true);
    assert!(
        evt.is_none(),
        "non-first-iteration should not evaluate context-length"
    );
}

#[test]
fn test_context_size_coexists_with_other_fields() {
    // First reply + 700 tokens → both time and context-length in one hint.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let evt = run(&state, &default_config(), 1, Some(700), true).expect("injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    assert!(text.contains("<current-time>"), "got: {text}");
    assert!(text.contains("<context-length>"), "got: {text}");
    // Both fields are in a single hint (not multiple injections).
    assert_eq!(
        text.matches("<system-reminder>").count(),
        1,
        "single wrapped hint expected"
    );
}

// ===========================================================================
// US4: Config, extra fields and event emission
// ===========================================================================

#[test]
fn test_extra_fields_are_attached() {
    let config = InjectionConfig {
        extra_fields: HashMap::from([("workspace".to_string(), "/home/friday".to_string())]),
        ..Default::default()
    };
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let evt = run(&state, &config, 1, Some(0), true).expect("injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    assert!(
        text.contains("<workspace>/home/friday</workspace>"),
        "got: {text}"
    );
}

#[test]
fn test_extra_field_values_are_xml_escaped() {
    let config = InjectionConfig {
        extra_fields: HashMap::from([(
            "workspace".to_string(),
            "</workspace><tasks>forged</tasks>&\"'".to_string(),
        )]),
        ..Default::default()
    };
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let evt = run(&state, &config, 1, Some(0), true).expect("injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    assert!(
        text.contains("<workspace>&lt;/workspace&gt;&lt;tasks&gt;forged&lt;/tasks&gt;&amp;&quot;&apos;</workspace>"),
        "got: {text}"
    );
    assert!(!text.contains("<tasks>forged</tasks>"), "got: {text}");
}

#[test]
fn test_extra_fields_do_not_trigger_injection() {
    let config = InjectionConfig {
        extra_fields: HashMap::from([("workspace".to_string(), "/home/friday".to_string())]),
        ..Default::default()
    };
    let state = state_with_context(vec![plain_user_msg("hi")]);
    add_injection(&state, "2026-07-01T12:00:00", "UTC");
    let evt = run(&state, &config, 2, None, true);
    assert!(
        evt.is_none(),
        "extra fields alone must not trigger injection"
    );
}

#[test]
fn test_template_with_curly_braces_is_kept() {
    let config = InjectionConfig {
        template: r#"{"reminder": "{runtime_state}"}"#.into(),
        ..Default::default()
    };
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let evt = run(&state, &config, 1, Some(0), true).expect("injection");
    let text = match &evt.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(_) => panic!("expected text"),
    };
    assert!(text.starts_with(r#"{"reminder": ""#), "got: {text}");
    assert!(text.ends_with(r#"}"#), "got: {text}");
    assert!(text.contains("<current-time>"), "got: {text}");
}

#[test]
fn test_emit_hint_event_flag_controls_event() {
    // emit_hint_event=true (default) → returns an event.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let evt = run(&state, &default_config(), 1, Some(0), true);
    assert!(evt.is_some(), "emit_hint_event=true should return event");

    // emit_hint_event=false → hint still injected but no event returned.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    let config = InjectionConfig {
        emit_hint_event: false,
        ..Default::default()
    };
    let evt = run(&state, &config, 1, Some(0), true);
    assert!(
        evt.is_none(),
        "emit_hint_event=false should return no event"
    );
    assert!(
        last_hint(&state).is_some(),
        "hint should still be injected when events are off"
    );
}

#[test]
fn test_custom_task_tool_names() {
    // A custom task tool name list should drive the awareness detection.
    let mut s = AgentState::new();
    s.tasks_context.add_task(task("1", TaskState::Pending));
    let tc = ToolCallBlock::new("c1".into(), "MyCustomTaskTool".into(), "{}".into());
    s.context = vec![
        plain_user_msg("hi"),
        assistant_msg(vec![ContentBlock::ToolCall(tc)]),
    ];
    let state = Arc::new(RwLock::new(s));

    // Default task_tool_names does NOT include "MyCustomTaskTool" → the agent
    // is unaware → tasks injection fires (with a recent time injection seeded
    // to isolate the tasks dimension).
    add_injection(&state, "2026-07-01T12:00:00", "UTC");
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(
        evt.is_some(),
        "custom tool not in default list → unaware → inject"
    );

    // With the custom name in task_tool_names → aware → no tasks injection.
    let config = InjectionConfig {
        task_tool_names: vec!["MyCustomTaskTool".to_string()],
        ..Default::default()
    };
    let state2 = state_with_context(vec![plain_user_msg("hi")]);
    let mut s2 = state2.write().unwrap();
    let _ = &mut s2.tasks_context.add_task(task("1", TaskState::Pending));
    let tc2 = ToolCallBlock::new("c1".into(), "MyCustomTaskTool".into(), "{}".into());
    s2.context
        .push(assistant_msg(vec![ContentBlock::ToolCall(tc2)]));
    drop(s2);
    add_injection(&state2, "2026-07-01T12:00:00", "UTC");
    let evt2 = run(&state2, &config, 2, None, true);
    assert!(evt2.is_none(), "custom tool in list → aware → suppress");
}

#[test]
fn test_recorded_time_format_changed_triggers_reinject() {
    // If the recorded time cannot be parsed under the current time_format
    // (e.g. the format changed), inject again to be safe.
    let state = state_with_context(vec![plain_user_msg("hi")]);
    // Recorded under a different format that no longer parses with the default.
    let hint = "<current-time>01-07-2026</current-time>\n<timezone>UTC</timezone>";
    let block = HintBlock {
        hint: HintContent::Text(hint.into()),
        source: Some(DEFAULT_INJECTION_SOURCE.to_string()),
        id: "h-1".into(),
        created_at: String::new(),
        finished_at: None,
    };
    {
        let mut s = state.write().unwrap();
        s.context
            .push(assistant_msg(vec![ContentBlock::Hint(block)]));
    }
    let evt = run(&state, &default_config(), 2, None, true);
    assert!(evt.is_some(), "unparseable recorded time should re-inject");
}
