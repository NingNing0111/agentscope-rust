//! Runtime-state injection pipeline — the unified implementation of the Python
//! `Agent._inject_runtime_state` (upstream commit `9d1026fa`).
//!
//! Evaluates three independent dimensions each reasoning iteration — current
//! time, unfinished tasks and context usage — and assembles the hits into a
//! **single** `HintBlock` appended to the persistent context (FR-013). When
//! `InjectionConfig.emit_hint_event` is enabled, a `HintBlockEvent` is returned
//! for the caller to emit over the event channel.
//!
//! The task dimension preserves Feature 024's byte-for-byte behavior (text,
//! source, awareness detection) so the existing `task_reminder` module can be
//! reduced to a thin wrapper over this pipeline.

use std::str::FromStr;
use std::sync::RwLock;

use agent_scope_event::{EventBase, HintBlockEvent};
use agent_scope_message::{ContentBlock, HintBlock, HintBlockItem, HintContent, Msg, Role};
use agent_scope_state::{AgentState, TaskState};

use crate::config::InjectionConfig;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Evaluate and inject runtime-state information into the agent context.
///
/// Evaluates the time, unfinished-task and context-length dimensions
/// independently. Returns a `HintBlockEvent` when an injection happened and
/// `InjectionConfig.emit_hint_event` is enabled; the caller emits it over the
/// event channel. The evaluation and the append happen under a single write
/// lock so concurrent tool execution cannot interleave a duplicate injection.
///
/// Parameters mirror the Python `_inject_runtime_state` inputs:
/// - `now`: the current wall-clock time in the configured timezone (caller
///   provides it so tests can freeze the clock).
/// - `cur_iter`: the 1-based iteration index of the current reply. The
///   context-length dimension is only evaluated on the first iteration
///   (`cur_iter == 1`, aligning with Python `state.cur_iter == 0`).
/// - `input_tokens`: the current input token count, only meaningful on the
///   first iteration. Pass `None` on later iterations (or when unknown) to
///   skip the context-length dimension.
/// - `context_size` / `trigger_ratio`: the model context window and the
///   compression trigger ratio, used to compute the context-length threshold.
/// - `task_tools_enabled`: Feature 024 flag; the task dimension is only
///   injected when this is also enabled (compat baseline, SC-002).
#[allow(clippy::too_many_arguments)]
pub fn maybe_inject_runtime_state(
    state: &RwLock<AgentState>,
    agent_name: &str,
    config: &InjectionConfig,
    now: chrono::DateTime<chrono::FixedOffset>,
    cur_iter: u32,
    input_tokens: Option<usize>,
    context_size: i64,
    trigger_ratio: f64,
    task_tools_enabled: bool,
) -> Option<HintBlockEvent> {
    if !config.inject_runtime_state {
        return None;
    }

    let mut state = state.write().unwrap_or_else(|e| e.into_inner());
    let reply_id = state.reply_context.reply_id.clone();

    // =====================================================================
    // Step 1: Analyze the current context
    //  - the latest injection that records a time (if any)
    //  - whether the agent is already aware of the uncompleted tasks
    // =====================================================================
    let mut pending = 0usize;
    let mut in_progress = 0usize;
    for task in &state.tasks_context.tasks {
        match task.state {
            TaskState::Pending => pending += 1,
            TaskState::InProgress => in_progress += 1,
            TaskState::Completed => {}
        }
    }
    let has_uncompleted_tasks = pending > 0 || in_progress > 0;

    // The text of the newest injection that records a time.
    let mut last_time_text: Option<String> = None;
    // The agent is aware of the tasks when the context contains the task
    // related tool calls or a previous tasks injection.
    let mut aware_of_tasks = !has_uncompleted_tasks;

    for msg in state.context.iter().rev() {
        if last_time_text.is_some() && aware_of_tasks {
            // Both dimensions are settled, no need to scan the older context.
            break;
        }
        if msg.role != Role::Assistant {
            continue;
        }
        for block in msg.content.iter().rev() {
            match block {
                ContentBlock::Hint(hb)
                    if hb.source.as_deref() == Some(config.injection_source.as_str()) =>
                {
                    let text = hint_text(hb);
                    if last_time_text.is_none() && text.contains("<current-time>") {
                        last_time_text = Some(text.clone());
                    }
                    if !aware_of_tasks && text.contains("<tasks>") {
                        aware_of_tasks = true;
                    }
                }
                ContentBlock::ToolCall(tc)
                    if !aware_of_tasks && config.task_tool_names.iter().any(|n| n == &tc.name) =>
                {
                    aware_of_tasks = true;
                }
                _ => {}
            }
        }
    }

    // =====================================================================
    // Step 2: Time dimension
    // =====================================================================
    let mut injections: Vec<(&'static str, String)> = Vec::new();

    // The wall-clock time in the configured timezone. The injected time must be
    // the local wall-clock of `config.timezone` (Python: `datetime.now(_resolve_timezone(...))`),
    // not the UTC instant carried by `now`. Fall back to UTC when the name is
    // unresolvable (aligned with Python `_resolve_timezone`).
    let tz = resolve_timezone(&config.timezone);
    let local_now = now.with_timezone(&tz);

    let timezone_text = config.timezone.clone();
    let inject_time = match &last_time_text {
        // No time recorded in the context — first reply or right after a
        // compression — so inject to be safe.
        None => true,
        Some(last_text) => {
            if !last_text_parses(last_text, config) {
                true
            } else {
                match extract_recorded_time(last_text, config) {
                    Some(last) => {
                        // Negative elapsed time means the recorded time is in the
                        // future (e.g. the machine clock went backwards) — inject
                        // again to be safe.
                        let elapsed_hours = (now - last).num_seconds() as f64 / 3600.0;
                        !(0.0..=config.time_interval).contains(&elapsed_hours)
                    }
                    None => true,
                }
            }
        }
    };
    if inject_time {
        injections.push((
            "current-time",
            local_now.format(&config.time_format).to_string(),
        ));
        injections.push(("timezone", timezone_text));
    }

    // =====================================================================
    // Step 3: Plan tasks dimension
    // =====================================================================
    // If uncompleted tasks exist and the agent isn't aware of them, inject a
    // reminder. The task dimension additionally requires `task_tools_enabled`
    // (Feature 024 compat baseline).
    if has_uncompleted_tasks && !aware_of_tasks && task_tools_enabled {
        injections.push((
            "tasks",
            format!(
                "You have {in_progress} in-progress tasks and {pending} pending tasks. Use `TaskList` to view them if you don't know."
            ),
        ));
    }

    // =====================================================================
    // Step 4: Context-length dimension (first iteration only)
    // =====================================================================
    if cur_iter == 1
        && let Some(input_tokens) = input_tokens
    {
        let buffer = config.context_buffer_ratio;
        let threshold = (0.0f64.max(trigger_ratio - buffer) * context_size as f64) as usize;
        if input_tokens > threshold {
            let trigger_tokens = (trigger_ratio * context_size as f64) as usize;
            injections.push((
                "context-length",
                format!(
                    "Your current context contains {input_tokens} tokens. When reaching {trigger_tokens} tokens, your context will be compressed."
                ),
            ));
        }
    }

    if injections.is_empty() {
        return None;
    }

    // The user defined fields are attached to every injection, but never
    // trigger one by themselves.
    let mut joined: Vec<String> = injections
        .into_iter()
        .map(|(k, v)| format!("<{k}>{v}</{k}>"))
        .collect();
    for (k, v) in &config.extra_fields {
        joined.push(format!("<{k}>{}</{k}>", escape_xml(v)));
    }
    let runtime_state = joined.join("\n");

    let hint = config.template.replace("{runtime_state}", &runtime_state);
    let block = HintBlock {
        hint: HintContent::Text(hint),
        source: Some(config.injection_source.clone()),
        id: agent_scope_utils::id::generate_id(),
        created_at: chrono::Utc::now().to_rfc3339(),
        finished_at: None,
    };

    let block_id = block.id.clone();
    let hint_content = block.hint.clone();

    if let Ok(msg) = Msg::new(
        agent_name.into(),
        vec![ContentBlock::Hint(block)],
        Role::Assistant,
    ) {
        state.context.push(msg);
    } else {
        return None;
    }

    if config.emit_hint_event {
        Some(HintBlockEvent {
            base: EventBase::new(),
            reply_id,
            block_id,
            source: Some(config.injection_source.clone()),
            hint: hint_content,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Time extraction helpers
// ---------------------------------------------------------------------------

/// Whether the last recorded time injection contains a parseable time under
/// the current `time_format`.
fn last_text_parses(last_text: &str, config: &InjectionConfig) -> bool {
    extract_recorded_time(last_text, config).is_some()
}

/// Extract the recorded time from the latest injection text, honoring the
/// recorded timezone. Returns `None` when the time cannot be parsed or the
/// timezone cannot be resolved.
fn extract_recorded_time(
    last_text: &str,
    config: &InjectionConfig,
) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    use chrono::TimeZone;

    // Extract <current-time>...</current-time>
    let time_str = tag_text(last_text, "current-time")?;
    let naive = chrono::NaiveDateTime::parse_from_str(time_str.trim(), &config.time_format).ok()?;

    // Extract <timezone>...</timezone>, falling back to the configured timezone.
    let tz_name = tag_text(last_text, "timezone")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| config.timezone.clone());

    // Interpret the recorded wall-clock in the recorded timezone. Using
    // `from_local_datetime` resolves the *actual* offset at the recorded date
    // (DST-aware), so the elapsed-time comparison holds even across DST
    // transitions or after the configured timezone changed mid-conversation
    // (Python: `last_time.replace(tzinfo=_resolve_timezone(...))`).
    let tz = resolve_timezone(&tz_name);
    tz.from_local_datetime(&naive)
        .single()
        .map(|dt| dt.fixed_offset())
}

/// Resolve an IANA timezone name to a `chrono_tz::Tz`. Unresolvable names fall
/// back to UTC (aligned with Python `_resolve_timezone`).
fn resolve_timezone(tz_name: &str) -> chrono_tz::Tz {
    match chrono_tz::Tz::from_str(tz_name) {
        Ok(tz) => tz,
        Err(_) => {
            tracing::warn!(
                timezone = %tz_name,
                "Failed to resolve timezone, fallback to UTC"
            );
            chrono_tz::UTC
        }
    }
}

fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Extract the inner text of the first `<tag>...</tag>` occurrence.
fn tag_text<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let rest = &text[start..];
    let end = rest.find(&close)?;
    Some(&rest[..end])
}

// ---------------------------------------------------------------------------
// Hint text extraction
// ---------------------------------------------------------------------------

/// Extract the plain-text content of a hint block.
fn hint_text(hb: &HintBlock) -> String {
    match &hb.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(items) => items
            .iter()
            .filter_map(|item| match item {
                HintBlockItem::Text(t) => Some(t.text.clone()),
                HintBlockItem::Data(_) => None,
            })
            .collect::<Vec<_>>()
            .join(""),
    }
}
