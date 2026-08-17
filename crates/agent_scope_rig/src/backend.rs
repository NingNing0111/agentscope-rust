//! RigChatBackend — 对象安全桥接 trait 与能力位。
//!
//! 本模块是 `agent_scope_rig` 内部架构的核心：把 rig 的**泛型** provider 类型
//! （[`rig::completion::CompletionResponse<T>`] / [`rig::streaming::StreamingCompletionResponse<R>`] /
//! [`rig::streaming::StreamedAssistantContent<R>`]）归一化为非泛型类型，使 trait 方法
//! 可对象安全（宪法第十一条：trait object）。
//!
//! **为什么需要归一化**：rig 0.41.0 的补全响应均携带 provider 原始响应（`T`/`R`），
//! `T`/`R` 无法在 `dyn RigChatBackend` 的方法签名中表达。因此：
//! - `completion()` 只保留非泛型字段（`Vec<AssistantContent>` + `Usage` + `message_id`）。
//! - `stream()` 返回自定义增量枚举 [`RigStreamDelta`]（无 `R`），由 backend 实现
//!   在消费 rig 原生流时归一化。
//!
//! 这是相对 research 决策 2 的实现偏离（research 按 0.42.0 非泛型签名设计），
//! 已登记于 [`specs/034-rig-llm-integration`] 偏差记录。

use std::pin::Pin;

use futures::Stream;
use rig::completion::message::{Reasoning, Text, ToolCall};
use rig::completion::{AssistantContent, CompletionRequest, Usage};
// ToolCallDeltaContent 在 rig-core `streaming` 模块（非 completion::message）。
use rig::streaming::ToolCallDeltaContent;

use agent_scope_model::model_error::ModelError;

/// Provider 家族标识。
///
/// 由各公开构造器（`RigChatModel::openai/anthropic/deepseek`）固定，用于
/// 能力位查表与错误映射的 `provider` 字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigProviderKind {
    OpenAi,
    Anthropic,
    DeepSeek,
}

impl RigProviderKind {
    /// provider 名（错误消息与 tracing 用）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::DeepSeek => "deepseek",
        }
    }
}

/// Provider 能力位（宪法第五条：显式登记，不静默）。
///
/// 定值见 `specs/034-rig-llm-integration/contracts/provider-adapter.md` §4 能力矩阵；
/// `thinking_tool_choice_incompatible` 的降级逻辑在 US4（T030）落地。
#[derive(Debug, Clone)]
pub struct RigProviderCapabilities {
    /// Provider 是否暴露 reasoning/thinking 内容（映射为 `ThinkingBlock`）。
    pub supports_thinking: bool,
    /// thinking 模式与 `tool_choice=required` 是否互斥。
    ///
    /// `true` 时 `RigChatModel::call_api` 遇到 `ToolChoice::required` 会降级为
    /// `auto` 并 `tracing::info!`（不静默），见 US4（T030）。
    pub thinking_tool_choice_incompatible: bool,
    /// Provider 是否提供 embedding 模型（仅 OpenAI 有 `text-embedding-3-*`）。
    pub supports_embedding: bool,
}

/// 归一化非流式补全结果。
///
/// rig 0.41.0 [`rig::completion::CompletionResponse<T>`] 的 `raw_response` 是
/// provider 原始响应，无法在 trait 对象中表达；只保留非泛型字段。
#[derive(Debug)]
pub struct NormCompletion {
    /// 归一化的 assistant 内容块（`OneOrMany` → `Vec`，非空）。
    pub choice: Vec<AssistantContent>,
    /// Token 用量。
    pub usage: Usage,
    /// Provider 分配的 message id（可能 `None`）。
    pub message_id: Option<String>,
}

/// 归一化流式 finish_reason。
///
/// rig 0.41.0 的 finish_reason 藏在 provider 原始流式响应（`R`）内部，无统一
/// 类型。backend 实现从各自的 `R` 提取后归一化为此枚举；无法提取时为 `None`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigStreamFinishReason {
    /// 正常完成（`stop`/`end_turn` 等）。
    Completed,
    /// 输出被截断（`length`/`max_tokens`/`content_filter` 等）。
    Interrupted,
}

/// 归一化流式增量。
///
/// rig 0.41.0 [`rig::streaming::StreamedAssistantContent<R>`] 是泛型，trait 对象
/// 无法直接返回；backend 实现消费 rig 原生流后映射为无泛型 delta。
///
/// `Final` 变体在流末生成：usage / finish_reason / message_id 在 rig 流消费完成后
/// 才可读（`StreamingCompletionResponse.response` / `message_id` / `usage()`）。
#[derive(Debug)]
pub enum RigStreamDelta {
    /// 文本增量（rig 每 chunk 一个完整 `Text`，消费侧按 `append_text` 增量拼接）。
    Text(Text),
    /// 完整工具调用。
    ToolCall {
        tool_call: ToolCall,
        /// rig 生成的内部调用 id（与 `ToolCallDelta.internal_call_id` 关联）。
        internal_call_id: String,
    },
    /// 工具调用增量（name / arguments 分段）。
    ToolCallDelta {
        /// Provider 工具调用 id。
        id: String,
        /// rig 生成的内部调用 id（稳定 block_id 拼接键）。
        internal_call_id: String,
        content: ToolCallDeltaContent,
    },
    /// 完整推理块。
    Reasoning(Reasoning),
    /// 推理增量。
    ReasoningDelta {
        /// Provider 推理块 id（可能 `None`）。
        id: Option<String>,
        reasoning: String,
    },
    /// 流末聚合事件（由 backend 生成，保证每个流恰好一个 `Final`）。
    Final {
        usage: Usage,
        finish_reason: Option<RigStreamFinishReason>,
        message_id: Option<String>,
    },
}

/// 对象安全聊天后端 trait。
///
/// 由各 provider backend（`OpenAiBackend`/`AnthropicBackend`/`DeepSeekBackend`）实现。
/// rig 类型仅在此 trait 边界内流转，`RigChatModel` 消费时已全部归一化为
/// agent_scope 既有类型（宪法第十二条）。
#[async_trait::async_trait]
pub trait RigChatBackend: Send + Sync {
    /// 能力位（定值见契约能力矩阵）。
    fn capabilities(&self) -> &RigProviderCapabilities;

    /// 非流式补全：`CompletionRequest` → 归一化结果。
    async fn completion(&self, request: CompletionRequest) -> Result<NormCompletion, ModelError>;

    /// 流式补全：返回归一化增量流（含流末 `Final`）。
    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<RigStreamDelta, ModelError>> + Send>>, ModelError>;
}

/// 对象安全 embedding 后端 trait。
///
/// 仅 OpenAI provider 实现（Anthropic/DeepSeek 无 embedding 模型，构造入口
/// 编译期不存在，见契约 §4 已知限制）。
#[async_trait::async_trait]
pub trait RigEmbeddingBackend: Send + Sync {
    /// 输出向量维度（`EmbeddingModelCard.dimensions`）。
    fn ndims(&self) -> u32;

    /// 批量文本嵌入，返回每条文本一个向量。
    async fn embed_texts(
        &self,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, agent_scope_embedding::error::EmbeddingError>;

    /// 单条文本嵌入。
    async fn embed_text(
        &self,
        text: &str,
    ) -> Result<Vec<f32>, agent_scope_embedding::error::EmbeddingError>;
}
