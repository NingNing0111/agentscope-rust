//! T013 — 流式增量转换器测试（`RigStreamDelta` 流 → `ChatResponse` 流）。
//!
//! 对照 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §5 顺序契约：
//! Reasoning→Thinking 增量、Text 增量、ToolCall/Delta 按 `internal_call_id`
//! 拼接、`is_last` 只在末 chunk、流末 usage/finished_reason/tool_call_id_map。

use std::pin::Pin;

use agent_scope_message::block::ContentBlock;
use agent_scope_model::model_error::ModelError;
use agent_scope_model::response::{ChatResponse, FinishedReason};
use agent_scope_rig::backend::{RigStreamDelta, RigStreamFinishReason};
use agent_scope_rig::stream::delta_stream_to_chat_stream;
use futures::{Stream, StreamExt};
use rig::completion::Usage;
use rig::completion::message::{Reasoning, Text, ToolCall, ToolFunction};
use rig::streaming::ToolCallDeltaContent;

/// 把 `RigStreamDelta` 列表包装为转换器要求的 `Pin<Box<dyn Stream>>`。
fn to_pinned(
    deltas: Vec<RigStreamDelta>,
) -> Pin<Box<dyn Stream<Item = Result<RigStreamDelta, ModelError>> + Send>> {
    Box::pin(futures::stream::iter(deltas.into_iter().map(Ok)))
}

/// 同步收集转换器产出。
fn collect(deltas: Vec<RigStreamDelta>) -> Vec<Result<ChatResponse, ModelError>> {
    let stream = delta_stream_to_chat_stream(to_pinned(deltas));
    futures::executor::block_on(stream.collect::<Vec<_>>())
}

fn text(s: &str) -> RigStreamDelta {
    RigStreamDelta::Text(Text::new(s))
}

fn reasoning(s: &str) -> RigStreamDelta {
    RigStreamDelta::Reasoning(Reasoning::new(s))
}

fn reasoning_delta(id: Option<&str>, s: &str) -> RigStreamDelta {
    RigStreamDelta::ReasoningDelta {
        id: id.map(str::to_string),
        reasoning: s.to_string(),
    }
}

fn tool_call(
    internal_call_id: &str,
    provider_id: &str,
    name: &str,
    args: serde_json::Value,
) -> RigStreamDelta {
    RigStreamDelta::ToolCall {
        tool_call: ToolCall::new(
            provider_id.to_string(),
            ToolFunction::new(name.to_string(), args),
        ),
        internal_call_id: internal_call_id.to_string(),
    }
}

fn tool_name_delta(internal_call_id: &str, provider_id: &str, name: &str) -> RigStreamDelta {
    RigStreamDelta::ToolCallDelta {
        id: provider_id.to_string(),
        internal_call_id: internal_call_id.to_string(),
        content: ToolCallDeltaContent::Name(name.to_string()),
    }
}

fn tool_args_delta(internal_call_id: &str, args: &str) -> RigStreamDelta {
    RigStreamDelta::ToolCallDelta {
        id: String::new(),
        internal_call_id: internal_call_id.to_string(),
        content: ToolCallDeltaContent::Delta(args.to_string()),
    }
}

fn final_delta(usage: Usage, finish: Option<RigStreamFinishReason>) -> RigStreamDelta {
    RigStreamDelta::Final {
        usage,
        finish_reason: finish,
        message_id: Some("resp-1".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Text 增量
// ---------------------------------------------------------------------------

#[test]
fn text_delta_yields_incremental_text_block() {
    let out = collect(vec![text("Hel")]);
    assert_eq!(out.len(), 2, "text delta + final");
    let resp = out[0].as_ref().unwrap();
    assert!(!resp.is_last);
    assert!(resp.id.is_empty(), "id must be cleared, got '{}'", resp.id);
    match &resp.content[0] {
        ContentBlock::Text(tb) => {
            assert_eq!(tb.text, "Hel");
            assert_eq!(tb.id, "text_0");
        }
        other => panic!("expected text block, got {other:?}"),
    }
}

#[test]
fn multiple_text_deltas_keep_stable_block_id() {
    let out = collect(vec![text("Hel"), text("lo")]);
    assert_eq!(out.len(), 3);
    let first = out[0].as_ref().unwrap();
    let second = out[1].as_ref().unwrap();
    assert!(matches!(&first.content[0], ContentBlock::Text(t) if t.id == "text_0"));
    assert!(matches!(&second.content[0], ContentBlock::Text(t) if t.id == "text_0"));
    assert!(matches!(&second.content[0], ContentBlock::Text(t) if t.text == "lo"));
}

// ---------------------------------------------------------------------------
// Reasoning 增量
// ---------------------------------------------------------------------------

#[test]
fn reasoning_maps_to_thinking_block() {
    let out = collect(vec![reasoning("think step 1")]);
    let resp = out[0].as_ref().unwrap();
    match &resp.content[0] {
        ContentBlock::Thinking(tb) => {
            assert_eq!(tb.thinking, "think step 1");
            assert_eq!(tb.id, "thinking_0");
        }
        other => panic!("expected thinking block, got {other:?}"),
    }
}

#[test]
fn reasoning_delta_appends_to_same_block() {
    let out = collect(vec![
        reasoning("step 1"),
        reasoning_delta(Some("r1"), " step 2"),
    ]);
    let first = out[0].as_ref().unwrap();
    let second = out[1].as_ref().unwrap();
    assert!(matches!(&first.content[0], ContentBlock::Thinking(t) if t.id == "thinking_0"));
    assert!(matches!(&second.content[0], ContentBlock::Thinking(t) if t.id == "thinking_0"));
    assert!(matches!(&second.content[0], ContentBlock::Thinking(t) if t.thinking == " step 2"));
}

// ---------------------------------------------------------------------------
// ToolCall / ToolCallDelta 拼接
// ---------------------------------------------------------------------------

#[test]
fn tool_call_maps_to_tool_call_block_with_tc_idx() {
    let out = collect(vec![tool_call(
        "c1",
        "call_1",
        "search",
        serde_json::json!({"q": "rust"}),
    )]);
    let resp = out[0].as_ref().unwrap();
    match &resp.content[0] {
        ContentBlock::ToolCall(tc) => {
            assert_eq!(tc.id, "tc_0");
            assert_eq!(tc.name, "search");
            assert_eq!(tc.input, r#"{"q":"rust"}"#);
        }
        other => panic!("expected tool call block, got {other:?}"),
    }
}

#[test]
fn tool_call_delta_name_and_args_join_same_block() {
    let out = collect(vec![
        tool_name_delta("c1", "call_1", "search"),
        tool_args_delta("c1", r#"{"q":""#),
        tool_args_delta("c1", r#""rust"}"#),
    ]);
    assert_eq!(out.len(), 4);
    // 三块工具增量各自独立 yield（拼接在引擎侧 StreamAccumulator 完成），
    // 但 block_id 必须稳定为 tc_0。只检查三个增量（末个为 content 空的 final）。
    for (i, item) in out.iter().take(3).enumerate() {
        let resp = item.as_ref().unwrap();
        match &resp.content[0] {
            ContentBlock::ToolCall(tc) => assert_eq!(tc.id, "tc_0", "chunk {i}"),
            other => panic!("expected tool call block in chunk {i}, got {other:?}"),
        }
    }
    // 首个 delta 携带 provider id。
    let first = out[0].as_ref().unwrap();
    assert_eq!(first.tool_call_id_map.get("tc_0").unwrap(), "call_1");
}

#[test]
fn multiple_tool_calls_assign_increasing_idx() {
    let out = collect(vec![
        tool_call("c1", "call_1", "search", serde_json::json!({})),
        tool_call("c2", "call_2", "calc", serde_json::json!({"expr": "1+1"})),
    ]);
    let first = out[0].as_ref().unwrap();
    let second = out[1].as_ref().unwrap();
    assert!(matches!(&first.content[0], ContentBlock::ToolCall(t) if t.id == "tc_0"));
    assert!(matches!(&second.content[0], ContentBlock::ToolCall(t) if t.id == "tc_1"));
}

#[test]
fn tool_call_delta_reuses_idx_from_full_tool_call() {
    // 完整 ToolCall 先到（分配 tc_0），随后 delta 复用同一 idx。
    let out = collect(vec![
        tool_call("c1", "call_1", "search", serde_json::json!({"q": "a"})),
        tool_args_delta("c1", r#", "limit": 5}"#),
    ]);
    assert!(matches!(
        &out[1].as_ref().unwrap().content[0],
        ContentBlock::ToolCall(t) if t.id == "tc_0"
    ));
}

// ---------------------------------------------------------------------------
// 流末收尾
// ---------------------------------------------------------------------------

fn usage() -> Usage {
    Usage {
        input_tokens: 12,
        output_tokens: 34,
        total_tokens: 46,
        cached_input_tokens: 2,
        cache_creation_input_tokens: 3,
        ..Default::default()
    }
}

#[test]
fn final_delta_produces_is_last_response_with_usage_and_map() {
    let out = collect(vec![
        text("hi"),
        tool_call("c1", "call_1", "search", serde_json::json!({})),
        final_delta(usage(), Some(RigStreamFinishReason::Completed)),
    ]);
    let last = out.last().unwrap().as_ref().unwrap();
    assert!(last.is_last, "final response must be is_last");
    assert!(
        last.content.is_empty(),
        "final response content must be empty"
    );
    let u = last.usage.as_ref().unwrap();
    assert_eq!(u.input_tokens, 12);
    assert_eq!(u.output_tokens, 34);
    assert_eq!(u.cache_input_tokens, 2);
    assert_eq!(u.cache_creation_input_tokens, 3);
    assert_eq!(last.finished_reason, FinishedReason::Completed);
    assert_eq!(last.tool_call_id_map.get("tc_0").unwrap(), "call_1");
    assert_eq!(
        last.metadata.get("message_id").unwrap(),
        &serde_json::json!("resp-1")
    );
}

#[test]
fn interrupted_finish_reason_is_propagated() {
    let out = collect(vec![
        text("partial"),
        final_delta(usage(), Some(RigStreamFinishReason::Interrupted)),
    ]);
    let last = out.last().unwrap().as_ref().unwrap();
    assert_eq!(last.finished_reason, FinishedReason::Interrupted);
}

#[test]
fn eof_without_final_still_emits_is_last() {
    // 无 Final 的自然结束（如取消）：EOF 处补发 is_last=true。
    let out = collect(vec![text("hi")]);
    assert_eq!(out.len(), 2);
    let last = out.last().unwrap().as_ref().unwrap();
    assert!(last.is_last);
    assert!(last.content.is_empty());
}

// ---------------------------------------------------------------------------
// 错误传播
// ---------------------------------------------------------------------------

#[test]
fn error_delta_terminates_stream() {
    let stream = delta_stream_to_chat_stream(Box::pin(futures::stream::iter(vec![
        Ok(RigStreamDelta::Text(Text::new("x"))),
        Err(ModelError::ApiError {
            status: 429,
            message: "rate limited".to_string(),
            provider: "openai".to_string(),
        }),
    ])));
    let out = futures::executor::block_on(stream.collect::<Vec<_>>());
    assert_eq!(out.len(), 2);
    assert!(out[0].is_ok());
    assert!(out[1].is_err());
    // 错误后不补发 is_last 收尾（流终止，引擎侧按 EOF 处理）。
}
