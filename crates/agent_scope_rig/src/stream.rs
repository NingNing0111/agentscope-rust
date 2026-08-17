//! `RigStreamDelta` 流 → `ChatResponse` 增量流转换器（T018）。
//!
//! [`RigChatBackend::stream`]（backend.rs）把 rig 原生流归一化为 [`RigStreamDelta`]
//! 增量；本模块再把它转换为 agent 引擎 `reply_stream` 可观察的
//! `Stream<Item = Result<ChatResponse, ModelError>>`。
//!
//! 产出语义与现有 DashScope SSE 解析一致（契约见
//! `specs/034-rig-llm-integration/contracts/rig-mapping.md` §5）：
//! - 每个内容 delta → 一个 `is_last=false` 的增量 `ChatResponse`，block_id 稳定
//!   （文本 `text_0`、推理 `thinking_0`、工具调用 `tc_{idx}`），增量拼接由
//!   `ChatResponse::append_*` 完成。
//! - 工具调用的 provider id 记录在增量的 `tool_call_id_map`（`tc_{idx}` → rig
//!   `ToolCall.id`），由引擎侧 `StreamAccumulator` 汇总并在收尾时应用。
//! - 流末（`Final` 或 EOF）→ 一个 `content` 为空的 `is_last=true` 响应，携带
//!   `usage` / `finished_reason` / 汇总的 `tool_call_id_map`（契约束一次写入）。

use std::collections::{HashMap, HashSet};
use std::pin::Pin;

use futures::{Stream, StreamExt};
use rig::completion::{GetTokenUsage, Usage};
use rig::streaming::{StreamedAssistantContent, ToolCallDeltaContent};

use agent_scope_model::model_error::ModelError;
use agent_scope_model::response::{ChatResponse, FinishedReason};
use agent_scope_model::usage::ChatUsage;

use crate::backend::{RigStreamDelta, RigStreamFinishReason};
use crate::error::map_completion_error;

/// rig 原生流式响应 → [`RigStreamDelta`] 增量流（backend 共享的归一化器）。
///
/// 各 provider backend（openai/anthropic/deepseek）的 `stream()` 都返回
/// `StreamingCompletionResponse<R>`，其中 `R` 是 provider 专用 usage 包装类型
/// （均实现 [`GetTokenUsage`]）。本函数把 `Stream<Item =
/// Result<StreamedAssistantContent<R>, CompletionError>>` 映射为无泛型
/// `RigStreamDelta`；`CompletionError` → `ModelError`（error.rs，`provider`
/// 用于错误消息标识）。
///
/// rig 的归一化流不暴露 finish_reason（`stream.response` 仅为 usage），故
/// `Final.finish_reason` 恒为 `None`（契约 §6："无法提取时为 None"）。
/// message_id 在流消费完成后从 `stream.message_id` 读取（rig 在聚合时捕获）。
pub(crate) fn stream_to_delta_stream<R>(
    stream: rig::streaming::StreamingCompletionResponse<R>,
    provider: &'static str,
) -> impl Stream<Item = Result<RigStreamDelta, ModelError>> + Send + 'static
where
    R: Clone + Unpin + GetTokenUsage + Send + 'static,
{
    async_stream::try_stream! {
        // usage 零哨兵：provider 未在流中给出 usage 时保持 `Usage::new()`。
        let mut final_usage = Usage::new();

        // pin_mut! 自行 move + shadow；之后 `stream` 是 Pin<&mut ...>。
        futures::pin_mut!(stream);
        while let Some(item) = stream.next().await {
            let item = item.map_err(|e| map_completion_error(&e, provider))?;
            match item {
                StreamedAssistantContent::Text(t) => yield RigStreamDelta::Text(t),
                StreamedAssistantContent::ToolCall {
                    tool_call,
                    internal_call_id,
                } => yield RigStreamDelta::ToolCall {
                    tool_call,
                    internal_call_id,
                },
                StreamedAssistantContent::ToolCallDelta {
                    id,
                    internal_call_id,
                    content,
                } => yield RigStreamDelta::ToolCallDelta {
                    id,
                    internal_call_id,
                    content,
                },
                // rig `StreamedAssistantContent::Reasoning` 是 tuple variant，内容
                // 已是 `rig::completion::message::Reasoning`，与 `RigStreamDelta` 一致。
                StreamedAssistantContent::Reasoning(r) => {
                    yield RigStreamDelta::Reasoning(r);
                }
                StreamedAssistantContent::ReasoningDelta { id, reasoning } => {
                    yield RigStreamDelta::ReasoningDelta { id, reasoning };
                }
                // 流末聚合：`Final(res)` 的 `res` 是 provider usage，经 GetTokenUsage
                // 归一化为 `rig::completion::Usage`（契约约定"每个流恰好一个 Final"）。
                StreamedAssistantContent::Final(res) => {
                    final_usage = res.token_usage();
                }
                StreamedAssistantContent::Unknown(_) => {
                    // 未建模的 provider 原生项（如 hosted-tool 结果）：忽略。
                }
            }
        }

        // 每流恰好一个 Final：usage 从 Final 事件或零哨兵取；message_id 从
        // `stream.message_id` 字段读（rig 在流消费期间捕获，无独立流事件）。
        yield RigStreamDelta::Final {
            usage: final_usage,
            finish_reason: None,
            message_id: stream.message_id.clone(),
        };
    }
}

/// `RigStreamDelta` 流 → `ChatResponse` 增量流（契约 §5）。
///
/// 输入必须是 `RigChatBackend::stream()` 的产物（含流末 `Final`）。若输入流
/// 未产生 `Final` 就自然结束（如取消），仍在 EOF 处补发 `is_last=true` 响应，
/// 保证引擎侧"EOF 无 is_last"路径也能正确收尾（streaming_reactor）。
pub fn delta_stream_to_chat_stream(
    delta_stream: Pin<Box<dyn Stream<Item = Result<RigStreamDelta, ModelError>> + Send>>,
) -> impl Stream<Item = Result<ChatResponse, ModelError>> + Send + 'static {
    async_stream::try_stream! {
        // 工具调用累积：internal_call_id（rig 关联键）→ tc_{idx}（稳定 block_id）。
        let mut internal_to_idx: HashMap<String, usize> = HashMap::new();
        let mut next_tool_idx = 0usize;
        // 已发过 ToolCallDelta 的 block：rig 在流末 finalize 的完整 ToolCall
        // 内容与该 block 的 delta 总和一致，若再次 append 会造成 arguments
        // 重复（e2e 实测 `{"city":"Beijing"}{"city":"Beijing"}`）。与 DashScope
        // 流式"只发增量"对齐，此处跳过重复的完整 ToolCall（P1 去重）。
        let mut seen_delta: HashSet<String> = HashSet::new();
        // 汇总的 provider 工具调用 id 映射（流末一次性写入）。
        let mut tool_call_id_map: HashMap<String, String> = HashMap::new();

        let mut final_usage: Option<ChatUsage> = None;
        let mut final_reason = FinishedReason::Completed;
        let mut final_message_id: Option<String> = None;

        let mut stream = delta_stream;
        while let Some(item) = stream.next().await {
            match item? {
                RigStreamDelta::Text(t) => {
                    let mut resp = ChatResponse::default();
                    // default() fabricates a random uuid id which would overwrite
                    // the real response id in the engine's StreamAccumulator.
                    resp.id.clear();
                    resp.append_text(&t.text, Some("text_0"));
                    yield resp;
                }
                RigStreamDelta::Reasoning(r) => {
                    let mut resp = ChatResponse::default();
                    resp.id.clear();
                    resp.append_thinking(&r.display_text(), Some("thinking_0"), HashMap::new());
                    yield resp;
                }
                RigStreamDelta::ReasoningDelta { reasoning, .. } => {
                    let mut resp = ChatResponse::default();
                    resp.id.clear();
                    resp.append_thinking(&reasoning, Some("thinking_0"), HashMap::new());
                    yield resp;
                }
                RigStreamDelta::ToolCall {
                    tool_call,
                    internal_call_id,
                } => {
                    let idx = tool_call_idx(
                        &mut internal_to_idx,
                        &mut next_tool_idx,
                        &internal_call_id,
                    );
                    let block_id = format!("tc_{idx}");
                    // rig 完整 ToolCall 携带 provider id；非空才登记。
                    if !tool_call.id.is_empty() {
                        tool_call_id_map.insert(block_id.clone(), tool_call.id.clone());
                    }
                    let mut resp = ChatResponse::default();
                    resp.id.clear();
                    // 增量响应也携带 provider id（引擎侧 StreamAccumulator 据此累积，
                    // 与 DashScope SSE 解析一致）。
                    if let Some(v) = tool_call_id_map.get(&block_id) {
                        resp.tool_call_id_map.insert(block_id.clone(), v.clone());
                    }
                    // 该 block 已通过 ToolCallDelta 累积：rig finalize 的完整
                    // ToolCall 内容与 delta 总和一致，跳过重复 append（去重）。
                    // 首 chunk 即完整 ToolCall（provider 单 chunk 完整）时才 append。
                    if !seen_delta.contains(&block_id) {
                        let args = tool_call.function.arguments.to_string();
                        resp.append_tool_call(
                            &block_id,
                            &tool_call.function.name,
                            &args,
                            HashMap::new(),
                        );
                    }
                    yield resp;
                }
                RigStreamDelta::ToolCallDelta {
                    id,
                    internal_call_id,
                    content,
                } => {
                    let idx = tool_call_idx(
                        &mut internal_to_idx,
                        &mut next_tool_idx,
                        &internal_call_id,
                    );
                    let block_id = format!("tc_{idx}");
                    // 登记：该 block 已通过 delta 累积，流末 finalize 的完整
                    // ToolCall 将跳过（去重，见 ToolCall 分支）。
                    seen_delta.insert(block_id.clone());
                    if !id.is_empty() {
                        tool_call_id_map
                            .entry(block_id.clone())
                            .or_insert_with(|| id.clone());
                    }
                    let mut resp = ChatResponse::default();
                    resp.id.clear();
                    if let Some(v) = tool_call_id_map.get(&block_id) {
                        resp.tool_call_id_map.insert(block_id.clone(), v.clone());
                    }
                    match content {
                        ToolCallDeltaContent::Name(name) => {
                            resp.append_tool_call(&block_id, &name, "", HashMap::new());
                        }
                        ToolCallDeltaContent::Delta(args) => {
                            resp.append_tool_call(&block_id, "", &args, HashMap::new());
                        }
                    }
                    yield resp;
                }
                RigStreamDelta::Final {
                    usage,
                    finish_reason,
                    message_id,
                } => {
                    final_usage = Some(rig_usage_to_chat_usage(&usage));
                    if let Some(fr) = finish_reason {
                        final_reason = match fr {
                            RigStreamFinishReason::Completed => FinishedReason::Completed,
                            RigStreamFinishReason::Interrupted => FinishedReason::Interrupted,
                        };
                    }
                    final_message_id = message_id;
                }
            }
        }

        // 流末：content 为空（引擎侧累积已包含全部增量），只带收尾元数据。
        // `id` 必须为空：default() 会捏造随机 uuid，若带到收尾 chunk 会被
        // StreamAccumulator 捕获，覆盖引擎侧从内容 chunk 累积的真实 id
        // （与 DashScope 流式 final 的 `resp.id.clear()` 一致，audit D5）。
        let mut final_resp = ChatResponse {
            id: String::new(),
            is_last: true,
            usage: final_usage,
            finished_reason: final_reason,
            tool_call_id_map,
            ..Default::default()
        };
        if let Some(mid) = final_message_id {
            final_resp
                .metadata
                .insert("message_id".to_string(), serde_json::json!(mid));
        }
        yield final_resp;
    }
}

/// 为 `internal_call_id` 分配/复用稳定的 `tc_{idx}` 序号。
fn tool_call_idx(
    internal_to_idx: &mut HashMap<String, usize>,
    next_tool_idx: &mut usize,
    internal_call_id: &str,
) -> usize {
    if let Some(&idx) = internal_to_idx.get(internal_call_id) {
        return idx;
    }
    let idx = *next_tool_idx;
    *next_tool_idx += 1;
    internal_to_idx.insert(internal_call_id.to_string(), idx);
    idx
}

/// rig `Usage`（u64）→ `ChatUsage`（i64），缓存用量透传。
/// `pub(crate)`：lib.rs 非流式 completion 路径复用同一转换。
pub(crate) fn rig_usage_to_chat_usage(u: &Usage) -> ChatUsage {
    ChatUsage {
        input_tokens: u.input_tokens as i64,
        output_tokens: u.output_tokens as i64,
        cache_creation_input_tokens: u.cache_creation_input_tokens as i64,
        cache_input_tokens: u.cached_input_tokens as i64,
        ..Default::default()
    }
}
