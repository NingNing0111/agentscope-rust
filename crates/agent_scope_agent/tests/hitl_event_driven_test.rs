//! Event-driven HITL confirmation tests — Feature 032.
//!
//! Verifies the pause → confirm → resume loop aligned with the Python
//! reference implementation:
//!   1. `RequireUserConfirmEvent` pauses the reply_stream (no denied fed
//!      back, no ReplyEnd yet); tool_call carries `state=asking`.
//!   2. Host injects `UserConfirmResultEvent`; engine matches by tool_call_id
//!      and resumes the same agent.
//!   3. `confirmed=false` produces a `DENIED` tool_result (no execution).
//!   4. Accepted `rules` are adopted so later calls of the same tool no longer
//!      ask (US3).
//!   5. `RequireExternalExecutionEvent` / `ExternalExecutionResultEvent`
//!      pause/resume for externally executed tools (US4).
//!   6. `UserInterruptEvent` ends an awaiting reply as INTERRUPTED, or is a
//!      silent no-op when idle (US5).
//!
//! Tests use deterministic mock models (constitution Article 6).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_scope_agent::event_input::EventInput;
use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, PermissionContext, PermissionRule, ReActAgent, ReActConfig,
};
use agent_scope_event::{
    AgentEvent, ConfirmResult, EventBase, ExternalExecutionResultEvent, UserConfirmResultEvent,
    UserInterruptEvent,
};
use agent_scope_message::{PermissionRule as MessagePermissionRule, factory::user_msg};
use agent_scope_message::{ToolCallBlock, ToolCallState, ToolOutput, ToolResultBlock};
use agent_scope_tool::{FunctionTool, ToolKit};
use agent_scope_types::ReplyFinishedReason;
use futures::StreamExt;

mod mocks;
use mocks::{ScriptedModel, ScriptedResponse};

/// A `counted_tool` that increments an atomic counter when invoked.
fn counted_tool(name: &str, calls: Arc<AtomicUsize>) -> FunctionTool {
    FunctionTool::new_with_schema(
        name,
        "Count invocations",
        serde_json::json!({
            "type": "object",
            "properties": {},
        }),
        move |_input| {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                "executed".to_string()
            }
        },
    )
}

/// An externally-executed tool that MUST NOT run in-process (US4).
fn external_tool(name: &str, calls: Arc<AtomicUsize>) -> FunctionTool {
    counted_tool(name, calls).with_external_execution()
}

/// Build an agent whose `dangerous_tool` is guarded by the given permission rule.
fn agent_with_scripted_permission(
    rule: PermissionRule,
    calls: Arc<AtomicUsize>,
    script: Vec<ScriptedResponse>,
) -> ReActAgent {
    let model = Arc::new(ScriptedModel::new("scripted", script));

    let mut toolkit = ToolKit::new();
    toolkit.register(counted_tool("dangerous_tool", calls));

    let mut permission_context = PermissionContext::default();
    permission_context.add_rule(rule);

    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(toolkit)
        .permission_context(permission_context)
        .build()
        .unwrap();

    ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap()
}

/// The standard two-step script: tool call, then a text reply.
fn agent_with_permission(rule: PermissionRule, calls: Arc<AtomicUsize>) -> ReActAgent {
    agent_with_scripted_permission(
        rule,
        calls,
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "dangerous_tool".into(),
                input: r#"{"cmd":"demo"}"#.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
    )
}

/// Build an agent with an externally-executed `external_tool` (US4).
fn agent_with_external_tool(calls: Arc<AtomicUsize>) -> ReActAgent {
    let model = Arc::new(ScriptedModel::new(
        "scripted",
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "external_tool".into(),
                input: r#"{"cmd":"demo"}"#.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
    ));

    let mut toolkit = ToolKit::new();
    toolkit.register(external_tool("external_tool", calls));

    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(toolkit)
        .build()
        .unwrap();

    ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap()
}

/// Drain a stream into a Vec of events.
async fn drain(
    stream: std::pin::Pin<Box<dyn futures::Stream<Item = AgentEvent> + Send>>,
) -> Vec<AgentEvent> {
    let mut stream = stream;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    events
}

fn find_confirm(events: &[AgentEvent]) -> &agent_scope_event::RequireUserConfirmEvent {
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::RequireUserConfirm(c) => Some(c),
            _ => None,
        })
        .expect("RequireUserConfirm 事件缺失")
}

/// Call `reply_stream_event` and return the error message string (the stream
/// type does not implement `Debug`, so `unwrap_err` is unavailable).
async fn reply_stream_event_err(agent: &ReActAgent, input: EventInput) -> String {
    match agent.reply_stream_event(input).await {
        Ok(_) => panic!("expected reply_stream_event to fail"),
        Err(e) => e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// US1 (T010/T011) — pause → confirm true → resume
// ---------------------------------------------------------------------------

/// T010: Ask 时 reply_stream 暂停（emit RequireUserConfirm 后流结束，
/// 不喂 denied 给模型、无 ReplyEnd），tool_call 带 state=asking 与
/// suggested_rules。
#[tokio::test]
async fn ask_pauses_stream_with_state_asking() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::ask("dangerous_tool"), calls);

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;

    let confirm = find_confirm(&events);
    assert_eq!(confirm.tool_calls.len(), 1);
    let tc = &confirm.tool_calls[0];
    assert_eq!(tc.name, "dangerous_tool");
    assert!(
        !events.iter().any(|e| matches!(e, AgentEvent::ReplyEnd(_))),
        "暂停时不应有 ReplyEnd"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolResultEnd(_))),
        "暂停时不应喂 denied tool_result"
    );
    // 事件里 tool_call 状态需为 asking（对齐 Python state="asking"）。
    assert!(
        matches!(tc.state, ToolCallState::Asking),
        "tool_call state 应为 asking"
    );
}

/// T011: 注入 `UserConfirmResultEvent{confirmed:true}` 恢复 → 工具执行 →
/// tool_result → ReplyEnd(completed)，同一 agent 从暂停点继续。
#[tokio::test]
async fn confirm_true_resumes_and_executes_tool() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::ask("dangerous_tool"), Arc::clone(&calls));

    // 1. First reply pauses.
    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let confirm = find_confirm(&events);
    assert_eq!(calls.load(Ordering::SeqCst), 0, "暂停时工具不得执行");

    // 2. Resume the SAME agent with confirmed=true.
    let resume_event = UserConfirmResultEvent {
        base: EventBase::new(),
        reply_id: confirm.reply_id.clone(),
        confirm_results: vec![ConfirmResult {
            confirmed: true,
            tool_call: confirm.tool_calls[0].clone(),
            rules: None,
        }],
    };
    let resume_events = drain(
        agent
            .reply_stream_event(EventInput::Confirm(resume_event))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(calls.load(Ordering::SeqCst), 1, "确认后工具应执行");
    assert!(
        resume_events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolResultEnd(_))),
        "恢复后应有 tool_result"
    );
    assert!(
        resume_events.iter().any(|e| matches!(
            e,
            AgentEvent::ReplyEnd(end) if end.finished_reason == ReplyFinishedReason::Completed
        )),
        "恢复后应以 ReplyEnd(completed) 结束"
    );
    // 恢复不应重新 emit ReplyStart（同一回复延续）。
    assert!(
        !resume_events
            .iter()
            .any(|e| matches!(e, AgentEvent::ReplyStart(_))),
        "恢复不应重新 ReplyStart"
    );
}

/// 拒绝（confirmed=false）：工具不执行，生成 DENIED tool_result（FR-006）。
#[tokio::test]
async fn confirm_false_denies_tool_without_execution() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::ask("dangerous_tool"), Arc::clone(&calls));

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let confirm = find_confirm(&events);

    let resume_event = UserConfirmResultEvent {
        base: EventBase::new(),
        reply_id: confirm.reply_id.clone(),
        confirm_results: vec![ConfirmResult {
            confirmed: false,
            tool_call: confirm.tool_calls[0].clone(),
            rules: None,
        }],
    };
    let resume_events = drain(
        agent
            .reply_stream_event(EventInput::Confirm(resume_event))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(calls.load(Ordering::SeqCst), 0, "拒绝的工具不得执行");
    assert!(
        resume_events.iter().any(|e| matches!(
            e,
            AgentEvent::ToolResultEnd(end) if end.state == agent_scope_message::ToolResultState::Denied
        )),
        "拒绝应生成 DENIED tool_result"
    );
}

// ---------------------------------------------------------------------------
// US2 (T014-T016) — invalid resume errors
// ---------------------------------------------------------------------------

/// T014: agent 无 awaiting 时注入确认事件 → 明确错误（FR-007）。
#[tokio::test]
async fn confirm_without_awaiting_errors() {
    let calls = Arc::new(AtomicUsize::new(0));
    // 用 allow 规则构建 agent，从未暂停 → 无 awaiting。
    let agent = agent_with_permission(PermissionRule::allow("dangerous_tool"), calls);

    let resume_event = UserConfirmResultEvent {
        base: EventBase::new(),
        reply_id: "reply-1".into(),
        confirm_results: vec![ConfirmResult {
            confirmed: true,
            tool_call: ToolCallBlock::new("tc1".into(), "dangerous_tool".into(), "{}".into()),
            rules: None,
        }],
    };
    let err = reply_stream_event_err(&agent, EventInput::Confirm(resume_event)).await;
    assert!(
        err.contains("not waiting for user confirmation"),
        "错误信息应指出 agent 未在等待确认: {err}"
    );
}

/// T015: 注入 id 与 awaiting 不匹配的确认结果 → 报错指出额外 id（FR-008）。
#[tokio::test]
async fn confirm_with_extra_tool_call_id_errors() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::ask("dangerous_tool"), calls);

    // 先暂停。
    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let confirm = find_confirm(&events);

    // 注入一个不存在的 tool_call id。
    let resume_event = UserConfirmResultEvent {
        base: EventBase::new(),
        reply_id: confirm.reply_id.clone(),
        confirm_results: vec![ConfirmResult {
            confirmed: true,
            tool_call: ToolCallBlock::new(
                "nonexistent".into(),
                "dangerous_tool".into(),
                "{}".into(),
            ),
            rules: None,
        }],
    };
    let err = reply_stream_event_err(&agent, EventInput::Confirm(resume_event)).await;
    assert!(
        err.contains("not waiting for confirmation"),
        "错误信息应指出额外 id: {err}"
    );
}

/// T016: reply_id 与暂停回复不匹配 → 报错（FR-010）。
#[tokio::test]
async fn confirm_with_wrong_reply_id_errors() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::ask("dangerous_tool"), calls);

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let confirm = find_confirm(&events);

    let resume_event = UserConfirmResultEvent {
        base: EventBase::new(),
        // 错误的 reply_id（与暂停回复不同）。
        reply_id: "wrong-reply".into(),
        confirm_results: vec![ConfirmResult {
            confirmed: true,
            tool_call: confirm.tool_calls[0].clone(),
            rules: None,
        }],
    };
    let err = reply_stream_event_err(&agent, EventInput::Confirm(resume_event)).await;
    assert!(
        err.contains("reply_id mismatch"),
        "错误信息应指出 reply_id 不匹配: {err}"
    );
}

// ---------------------------------------------------------------------------
// US3 (T019) — ConfirmResult.rules 采纳进引擎
// ---------------------------------------------------------------------------

/// T019: 确认结果携带 `rules:[allow(tool)]` → 恢复后同工具再次调用直接放行。
#[tokio::test]
async fn confirm_with_rules_adopts_and_allows_future_calls() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_scripted_permission(
        PermissionRule::ask("dangerous_tool"),
        Arc::clone(&calls),
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "dangerous_tool".into(),
                input: r#"{"cmd":"demo"}"#.into(),
            },
            // 第二次调用同工具：若 allow 规则已采纳则不再 Ask。
            ScriptedResponse::ToolCall {
                id: "tc2".into(),
                name: "dangerous_tool".into(),
                input: r#"{"cmd":"demo"}"#.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
    );

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let confirm = find_confirm(&events);

    // 构建消息级 allow 规则（扁平 extras，序列化后可解码为引擎规则）。
    let mut extras = HashMap::new();
    extras.insert("tool_name".into(), serde_json::json!("dangerous_tool"));
    extras.insert("behavior".into(), serde_json::json!("allow"));
    extras.insert("source".into(), serde_json::json!("runtime"));
    let rule = MessagePermissionRule { extras };

    let resume_event = UserConfirmResultEvent {
        base: EventBase::new(),
        reply_id: confirm.reply_id.clone(),
        confirm_results: vec![ConfirmResult {
            confirmed: true,
            tool_call: confirm.tool_calls[0].clone(),
            rules: Some(vec![rule]),
        }],
    };
    let resume_events = drain(
        agent
            .reply_stream_event(EventInput::Confirm(resume_event))
            .await
            .unwrap(),
    )
    .await;

    // 两次调用都执行（第二次因 allow 规则直接放行，不再暂停）。
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "采纳 allow 规则后应放行第二次调用"
    );
    assert!(
        !resume_events
            .iter()
            .any(|e| matches!(e, AgentEvent::RequireUserConfirm(_))),
        "第二次调用不应再触发确认"
    );
    assert!(
        resume_events.iter().any(|e| matches!(
            e,
            AgentEvent::ReplyEnd(end) if end.finished_reason == ReplyFinishedReason::Completed
        )),
        "应以 ReplyEnd(completed) 结束"
    );
}

// ---------------------------------------------------------------------------
// US4 (T021-T023) — external execution pause/resume
// ---------------------------------------------------------------------------

/// T021: 工具触发 `RequireExternalExecutionEvent`（携带 tool_calls）→ 流结束暂停。
#[tokio::test]
async fn external_tool_emits_require_external_execution_and_pauses() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_external_tool(Arc::clone(&calls));

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;

    let ext = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::RequireExternalExecution(ee) => Some(ee),
            _ => None,
        })
        .expect("RequireExternalExecution 事件缺失");
    assert_eq!(ext.tool_calls.len(), 1);
    assert_eq!(ext.tool_calls[0].name, "external_tool");
    assert!(
        !events.iter().any(|e| matches!(e, AgentEvent::ReplyEnd(_))),
        "外部执行暂停时不应有 ReplyEnd"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0, "外部工具不得在进程内执行");
    // tool_call 状态应为 submitted（等待外部结果）。
    let state = agent.state();
    assert!(
        state.context.iter().any(|m| m.content.iter().any(|b| {
            matches!(
                b,
                agent_scope_message::ContentBlock::ToolCall(tc)
                    if tc.state == ToolCallState::Submitted
            )
        })),
        "外部执行的 tool_call 应为 submitted"
    );
}

/// T022: 注入 `ExternalExecutionResultEvent` → 结果追加 context、agent 继续。
#[tokio::test]
async fn external_execution_result_resumes_and_continues() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_external_tool(calls);

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let ext = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::RequireExternalExecution(ee) => Some(ee),
            _ => None,
        })
        .expect("RequireExternalExecution 事件缺失");

    let resume_event = ExternalExecutionResultEvent {
        base: EventBase::new(),
        reply_id: ext.reply_id.clone(),
        execution_results: vec![ToolResultBlock::new(
            "tc1".into(),
            "external_tool".into(),
            ToolOutput::Text("external-done".into()),
        )],
    };
    let resume_events = drain(
        agent
            .reply_stream_event(EventInput::ExternalResult(resume_event))
            .await
            .unwrap(),
    )
    .await;

    // 结果追加到 context。
    let state = agent.state();
    assert!(
        state.context.iter().any(|m| m.content.iter().any(|b| {
            matches!(
                b,
                agent_scope_message::ContentBlock::ToolResult(tr)
                    if tr.id == "tc1"
            )
        })),
        "外部执行结果应追加到 context"
    );
    // 工具状态 finished。
    assert!(
        state.context.iter().any(|m| m.content.iter().any(|b| {
            matches!(
                b,
                agent_scope_message::ContentBlock::ToolCall(tc)
                    if tc.id == "tc1" && tc.state == ToolCallState::Finished
            )
        })),
        "外部执行后工具状态应为 finished"
    );
    // agent 继续 → model 返回 Text("done") → completed。
    assert!(
        resume_events.iter().any(|e| matches!(
            e,
            AgentEvent::ReplyEnd(end) if end.finished_reason == ReplyFinishedReason::Completed
        )),
        "外部执行恢复后应以 ReplyEnd(completed) 结束"
    );
}

/// T023: 外部执行结果 id 与等待状态不匹配 → 报错（FR-015）。
#[tokio::test]
async fn external_execution_result_with_wrong_id_errors() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_external_tool(calls);

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let ext = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::RequireExternalExecution(ee) => Some(ee),
            _ => None,
        })
        .expect("RequireExternalExecution 事件缺失");

    let resume_event = ExternalExecutionResultEvent {
        base: EventBase::new(),
        reply_id: ext.reply_id.clone(),
        execution_results: vec![ToolResultBlock::new(
            "nonexistent".into(),
            "external_tool".into(),
            ToolOutput::Text("x".into()),
        )],
    };
    let err = reply_stream_event_err(&agent, EventInput::ExternalResult(resume_event)).await;
    assert!(
        err.contains("not waiting for external execution"),
        "错误信息应指出外部执行 id 不匹配: {err}"
    );
}

// ---------------------------------------------------------------------------
// US5 (T027/T028) — user interrupt
// ---------------------------------------------------------------------------

/// T027: 有 awaiting 时注入 `UserInterruptEvent` → `ReplyEnd(INTERRUPTED)`。
#[tokio::test]
async fn interrupt_with_awaiting_ends_interrupted() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::ask("dangerous_tool"), calls);

    let events = drain(
        agent
            .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
            .await
            .unwrap(),
    )
    .await;
    let confirm = find_confirm(&events);

    let interrupt = UserInterruptEvent {
        base: EventBase::new(),
        reply_id: confirm.reply_id.clone(),
    };
    let events = drain(
        agent
            .reply_stream_event(EventInput::Interrupt(interrupt))
            .await
            .unwrap(),
    )
    .await;

    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentEvent::ReplyEnd(end) if end.finished_reason == ReplyFinishedReason::Interrupted
        )),
        "有 awaiting 时中断应以 ReplyEnd(INTERRUPTED) 结束"
    );
}

/// T028: 无 awaiting 时注入中断 → 静默 no-op。
#[tokio::test]
async fn interrupt_without_awaiting_is_silent_noop() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::allow("dangerous_tool"), calls);

    let interrupt = UserInterruptEvent {
        base: EventBase::new(),
        reply_id: "reply-1".into(),
    };
    let events = drain(
        agent
            .reply_stream_event(EventInput::Interrupt(interrupt))
            .await
            .unwrap(),
    )
    .await;

    assert!(
        events.is_empty(),
        "无 awaiting 时中断应为静默 no-op（无任何事件）"
    );
}
