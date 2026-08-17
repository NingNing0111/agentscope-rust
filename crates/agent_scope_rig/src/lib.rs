//! agent_scope_rig — rig-backed `ChatModel`/`EmbeddingModel` 适配 crate。
//!
//! Feature 034：用 rig LLM 框架接入 OpenAI/Anthropic/DeepSeek。公开入口为
//! [`RigChatModel`]（聊天）与
//! [`RigEmbeddingModel`]（OpenAI embedding，T021）。
//!
//! 架构（契约见 `specs/034-rig-llm-integration/contracts/provider-adapter.md`）：
//! - [`RigChatBackend`]（backend.rs）是对象安全桥接 trait：rig 的**泛型** provider
//!   类型在此边界内归一化为 [`NormCompletion`]/[`RigStreamDelta`]（宪法第十一/十二条）。
//! - provider backend（`openai`/`anthropic`/`deepseek` 模块）实现该 trait。
//! - [`RigChatModel`] 持有 `Arc<dyn RigChatBackend>` + 配置，实现
//!   `agent_scope_model::ChatModel`；rig 类型不越过 crate 公开边界。

pub mod anthropic;
pub mod backend;
pub mod deepseek;
pub mod error;
pub mod message;
pub mod openai;
pub mod params;
pub mod stream;
pub mod structured;
pub mod tools;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use rig::completion::CompletionRequest;
use rig::core::OneOrMany;

use agent_scope_embedding::embedding::{
    EmbeddingInput, EmbeddingModel as EmbeddingModelTrait, EmbeddingModelCard, EmbeddingResponse,
    EmbeddingUsage,
};
use agent_scope_embedding::error::EmbeddingError;
use agent_scope_message::Msg;
use agent_scope_model::formatter::FormatError;
use agent_scope_model::model_error::{ModelError, ModelErrorKind};
use agent_scope_model::model_trait::{ChatModel, ModelCallResult};
use agent_scope_model::response::{ChatResponse, FinishedReason, StructuredResponse};
use agent_scope_model::schema_flat::flatten_json_schema_with_defs_checked;
use agent_scope_model::tool_choice::ToolChoice;

use crate::backend::{RigChatBackend, RigEmbeddingBackend, RigProviderKind};
use crate::message::{assistant_content_to_blocks, msg_to_rig_messages};
use crate::params::{RigParameters, apply_params};
use crate::stream::{delta_stream_to_chat_stream, rig_usage_to_chat_usage};
use crate::structured::{
    bypass_output_request, extract_structured, extract_tool_bypass, native_output_request,
};
use crate::tools::{filter_tool_definitions, json_schema_to_tool_definitions, tool_choice_to_rig};

/// `RigChatModel` 运行配置（链式构造后固化，默认值见契约 §1 等价表）。
///
/// 默认：`stream=true`、`max_retries=3`、`retry_delay=1.0`、
/// `retryable_errors=[ApiConnection, ApiTimeout, RateLimit, InternalServer]`。
/// `context_size` 由构造器按 provider 覆盖（OpenAI 131072 / Anthropic 200000 /
/// DeepSeek 131072，契约 §1 默认值等价表）。
#[derive(Debug, Clone)]
pub struct RigChatModelConfig {
    /// 是否默认流式（`stream_enabled()` 返回值）。
    pub stream: bool,
    /// 可选 base_url 覆盖（构造时或 `.with_base_url()` 设置；backend 首次使用
    /// 时校验合法性，非法 → `ValidationError`）。
    pub base_url: Option<String>,
    /// 生成参数（`max_tokens`/`temperature`/`top_p`/… → rig `CompletionRequest`）。
    pub parameters: RigParameters,
    /// 最大重试次数（`ChatModel::max_retries`）。
    pub max_retries: u32,
    /// 重试间隔秒数（`ChatModel::retry_delay`）。
    pub retry_delay: f64,
    /// 上下文窗口（`ChatModel::context_size`）。
    pub context_size: i64,
    /// 触发重试的错误分类（`ChatModel::retryable_errors`）。
    pub retryable_errors: Vec<ModelErrorKind>,
}

impl Default for RigChatModelConfig {
    fn default() -> Self {
        Self {
            stream: true,
            base_url: None,
            parameters: RigParameters::default(),
            max_retries: 3,
            retry_delay: 1.0,
            context_size: 131072,
            retryable_errors: vec![
                ModelErrorKind::ApiConnection,
                ModelErrorKind::ApiTimeout,
                ModelErrorKind::RateLimit,
                ModelErrorKind::InternalServer,
            ],
        }
    }
}

/// rig-backed 聊天模型。
///
/// 持有 provider backend 的 `Arc` trait 对象 + 配置。backend 懒构建（首次调用
/// `call_api` 时按 `provider`/`api_key`/`model`/`config.base_url` 构建），使
/// `.with_base_url()` 在构造后仍生效（每次更新配置并清空已构建 backend，下次
/// 调用重建）——`with_base_url` 语义与旧 provider crate 一致
/// （契约 §1 等价表"同名同语义"）。
///
/// `api_key` 私有存储（构造校验非空），不实现 `Debug` 显示以避免泄露。
pub struct RigChatModel {
    provider: RigProviderKind,
    model_name: String,
    config: RigChatModelConfig,
    api_key: String,
    backend: Mutex<Option<Arc<dyn RigChatBackend>>>,
}

impl std::fmt::Debug for RigChatModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigChatModel")
            .field("provider", &self.provider.as_str())
            .field("model_name", &self.model_name)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl RigChatModel {
    /// OpenAI 后端（Chat Completions，示例默认）。
    ///
    /// `api_key` 必填非空（空白 → `ValidationError`）。`base_url` 默认官方端点，
    /// 用 `.with_base_url()` 覆盖（backend 构建时校验合法性）。
    pub fn openai(
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ModelError> {
        Self::new(RigProviderKind::OpenAi, api_key, model)
    }

    /// Anthropic 后端（Messages API）。
    pub fn anthropic(
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ModelError> {
        Self::new(RigProviderKind::Anthropic, api_key, model)
    }

    /// DeepSeek 后端（OpenAI-compatible）。
    pub fn deepseek(
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ModelError> {
        Self::new(RigProviderKind::DeepSeek, api_key, model)
    }

    /// 构造：校验 api_key 非空，按 provider 定默认 `context_size`（契约 §1）。
    fn new(
        provider: RigProviderKind,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ModelError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(ModelError::ValidationError {
                field: "api_key".to_string(),
                message: "api_key must not be empty".to_string(),
            });
        }
        let context_size = match provider {
            RigProviderKind::OpenAi => 131072,
            RigProviderKind::Anthropic => 200000,
            RigProviderKind::DeepSeek => 131072,
        };
        Ok(Self {
            provider,
            model_name: model.into(),
            config: RigChatModelConfig {
                context_size,
                ..Default::default()
            },
            api_key,
            backend: Mutex::new(None),
        })
    }

    // ── 链式配置（契约 §1） ──────────────────────────────────────────

    /// 设置默认流式模式。
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.config.stream = stream;
        self
    }

    /// 覆盖 base_url（backend 首次使用时校验合法 URL）。
    ///
    /// 更新配置并清空已构建 backend，使后续调用按新 URL 重建（语义对齐
    /// 旧 provider crate 的 `with_base_url`）。
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.config.base_url = Some(base_url.into());
        if let Ok(mut guard) = self.backend.lock() {
            *guard = None;
        }
        self
    }

    /// 设置生成参数（`max_tokens`/`temperature`/`top_p`/…）。
    pub fn with_parameters(mut self, parameters: RigParameters) -> Self {
        self.config.parameters = parameters;
        self
    }

    /// 设置最大重试次数。
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// 设置重试间隔秒数。
    pub fn with_retry_delay(mut self, retry_delay: f64) -> Self {
        self.config.retry_delay = retry_delay;
        self
    }

    /// 设置上下文窗口大小。
    pub fn with_context_size(mut self, context_size: i64) -> Self {
        self.config.context_size = context_size;
        self
    }

    // ── backend 懒构建 ────────────────────────────────────────────────

    /// 取（或构建）provider backend。
    ///
    /// 首次调用按当前配置构建并缓存；`with_base_url` 已清空缓存 → 重建。
    /// 构建失败（api_key 非空已在上层校验，此处主要是 base_url 非法）→
    /// `ValidationError`，不污染缓存（下次调用重试）。
    fn backend(&self) -> Result<Arc<dyn RigChatBackend>, ModelError> {
        let mut guard = self
            .backend
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(b) = guard.as_ref() {
            return Ok(b.clone());
        }
        let built = build_backend(
            self.provider,
            &self.api_key,
            &self.model_name,
            self.config.base_url.as_deref(),
        )?;
        let arc: Arc<dyn RigChatBackend> = built;
        *guard = Some(arc.clone());
        Ok(arc)
    }

    /// 测试注入构造器：直接放入预构建 backend（跳过懒构建）。
    ///
    /// `#[doc(hidden)]`：仅供 `tests/*.rs` 集成测试 mock `RigChatBackend`
    /// 验证 `call_api`/`generate_structured_output` 编排；生产路径不走此入口。
    #[doc(hidden)]
    pub fn from_backend_for_testing(
        provider: RigProviderKind,
        model_name: impl Into<String>,
        backend: Arc<dyn RigChatBackend>,
    ) -> Self {
        Self {
            provider,
            model_name: model_name.into(),
            config: RigChatModelConfig::default(),
            api_key: String::new(),
            backend: Mutex::new(Some(backend)),
        }
    }
}

/// 按 provider 构建对应 backend（trait 对象）。
fn build_backend(
    kind: RigProviderKind,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
) -> Result<Arc<dyn RigChatBackend>, ModelError> {
    let base_url = base_url.map(str::to_string);
    match kind {
        RigProviderKind::OpenAi => Ok(Arc::new(crate::openai::OpenAiBackend::new(
            api_key, model, base_url,
        )?)),
        RigProviderKind::Anthropic => Ok(Arc::new(crate::anthropic::AnthropicBackend::new(
            api_key, model, base_url,
        )?)),
        RigProviderKind::DeepSeek => Ok(Arc::new(crate::deepseek::DeepSeekBackend::new(
            api_key, model, base_url,
        )?)),
    }
}

#[async_trait::async_trait]
impl ChatModel for RigChatModel {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn stream_enabled(&self) -> bool {
        self.config.stream
    }

    fn max_retries(&self) -> u32 {
        self.config.max_retries
    }

    fn retry_delay(&self) -> f64 {
        self.config.retry_delay
    }

    fn context_size(&self) -> i64 {
        self.config.context_size
    }

    fn retryable_errors(&self) -> &[ModelErrorKind] {
        &self.config.retryable_errors
    }

    async fn call_api(
        &self,
        _model_name: &str,
        messages: &[Msg],
        tools: Option<&[serde_json::Value]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        let backend = self.backend()?;

        // ── 出站映射（message.rs T005 / tools.rs T007） ───────────────
        let rig_messages = msg_to_rig_messages(messages)?;
        let chat_history = OneOrMany::from_iter_optional(rig_messages).ok_or_else(|| {
            ModelError::ValidationError {
                field: "messages".to_string(),
                message: "messages must not be empty".to_string(),
            }
        })?;

        // 工具定义：schema → rig ToolDefinition；`ToolChoice.tools` 子集过滤
        // （round-4 M18 语义，契约 §2.2）。
        let mut tool_defs = Vec::new();
        if let Some(schemas) = tools {
            tool_defs = json_schema_to_tool_definitions(schemas)?;
        }
        let tool_filter = tool_choice.and_then(|tc| tc.tools.as_deref());
        let tool_defs = filter_tool_definitions(&tool_defs, tool_filter);

        // US4（T030）：thinking 与 `tool_choice=required` 互斥的 provider 能力位。
        // Anthropic extended thinking 与 tool use 并发受约束（rig 0.41 +
        // anthropic provider：思考预算下强制 tool_choice，二者不可并发）——
        // 能力位 `thinking_tool_choice_incompatible=true` 由 backend 定值。
        // 降级为 `auto` 并 `tracing::info!`（宪法第五条：不静默），同时保留
        // `ToolChoice.tools` 子集过滤语义（`auto` 下 tools 子集仍可提示倾向）。
        let mut rig_tool_choice = tool_choice.and_then(tool_choice_to_rig);
        if backend.capabilities().thinking_tool_choice_incompatible
            && matches!(
                rig_tool_choice,
                Some(rig::completion::message::ToolChoice::Required)
            )
        {
            tracing::info!(
                provider = %self.provider.as_str(),
                "thinking_tool_choice_incompatible: degraded tool_choice=required -> auto \
                 (provider cannot run extended thinking and forced tool use concurrently)"
            );
            rig_tool_choice = Some(rig::completion::message::ToolChoice::Auto);
        }

        // CompletionRequest 不 impl Default，逐字段构造（rig-mapping.md §3）。
        let request = apply_params(
            CompletionRequest {
                model: Some(self.model_name.clone()),
                preamble: None,
                chat_history,
                documents: Vec::new(),
                tools: tool_defs,
                temperature: None,
                max_tokens: None,
                tool_choice: rig_tool_choice,
                additional_params: None,
                output_schema: None,
                record_telemetry_content: false,
            },
            &self.config.parameters,
        );

        // ── 分派（契约 §2） ───────────────────────────────────────────
        let start = Instant::now();
        if self.config.stream {
            let delta_stream = backend.stream(request).await?;
            let chat_stream = delta_stream_to_chat_stream(delta_stream);
            Ok(ModelCallResult::Stream(Box::pin(chat_stream)))
        } else {
            let norm = backend.completion(request).await?;
            let blocks = assistant_content_to_blocks(norm.choice)?;
            let mut usage = rig_usage_to_chat_usage(&norm.usage);
            usage.time = start.elapsed().as_secs_f64();

            let mut resp = ChatResponse {
                content: blocks,
                is_last: true,
                // provider message_id 优先；无则空串（不捏造随机 uuid，D5 原则）。
                id: norm.message_id.clone().unwrap_or_default(),
                usage: Some(usage),
                finished_reason: FinishedReason::Completed,
                ..Default::default()
            };
            if let Some(mid) = norm.message_id {
                resp.metadata
                    .insert("message_id".to_string(), serde_json::json!(mid));
            }
            Ok(ModelCallResult::Complete(resp))
        }
    }

    /// 结构化输出覆写（T019，契约 rig-mapping.md §6）。
    ///
    /// 优先 `CompletionRequest.output_schema` 原生路径（provider 支持时返回
    /// 严格 schema 约束的 JSON）；以下情况回退工具 bypass：
    /// 1. 原生请求被 provider 拒绝（400/422 `BadRequest`）；
    /// 2. 原生路径成功但响应无可解析 JSON（`extract_structured` 失败）；
    /// 3. JSON Schema 无法转换为 schemars `Schema`（`native_output_request` 失败）。
    ///
    /// 回退注入 `generate_structured_output` 工具 + `tool_choice=required`，
    /// 从工具调用 arguments 提取 JSON（`json_repair` 兜底）。空消息 →
    /// `ValidationError`（契约 §6.3）。结构化为非流式（原生 `output_schema`
    /// 走 `response_format`，需要完整响应）。
    async fn generate_structured_output(
        &self,
        messages: &[Msg],
        structured_model: &serde_json::Value,
    ) -> Result<StructuredResponse, ModelError> {
        if messages.is_empty() {
            return Err(ModelError::ValidationError {
                field: "messages".to_string(),
                message: "messages list must not be empty for structured output".to_string(),
            });
        }

        let json_schema = flatten_json_schema_with_defs_checked(structured_model).map_err(|e| {
            ModelError::FormatError {
                context: "structured_output".to_string(),
                source: FormatError::InvalidMessage(e.reason),
            }
        })?;

        let backend = self.backend()?;

        // ── 原生路径：CompletionRequest.output_schema ───────────────────
        match native_output_request(
            &self.model_name,
            messages,
            &json_schema,
            &self.config.parameters,
        ) {
            Ok(request) => match backend.completion(request).await {
                Ok(norm) => {
                    // 成功但未产出可解析 JSON → 回退 bypass。
                    if let Ok(resp) = extract_structured(&norm.choice, &norm.usage, norm.message_id)
                    {
                        return Ok(resp);
                    }
                    tracing::warn!(
                        provider = self.provider.as_str(),
                        "rig structured: native output_schema path yielded no parseable JSON; \
                         falling back to tool bypass"
                    );
                }
                // provider 拒绝原生 output_schema（400/422）→ 回退 bypass。
                Err(e) if matches!(e.kind(), Some(ModelErrorKind::BadRequest)) => {
                    tracing::warn!(
                        provider = self.provider.as_str(),
                        "rig structured: provider rejected output_schema ({e}); \
                         falling back to tool bypass"
                    );
                }
                Err(e) => return Err(e),
            },
            // JSON Schema 无法转 schemars Schema → 回退 bypass。
            Err(e) => {
                tracing::warn!(
                    provider = self.provider.as_str(),
                    "rig structured: output_schema not representable ({e}); \
                     falling back to tool bypass"
                );
            }
        }

        // ── 回退路径：generate_structured_output 工具 + required ───────
        let request = bypass_output_request(
            &self.model_name,
            messages,
            &json_schema,
            &self.config.parameters,
        )?;
        let norm = backend.completion(request).await?;
        extract_tool_bypass(&norm.choice, &norm.usage, norm.message_id)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Embedding（US3，T021）
// ═════════════════════════════════════════════════════════════════════════

/// rig-backed embedding 模型（OpenAI Embeddings API）。
///
/// 包装 `Arc<dyn RigEmbeddingBackend>`（对象安全桥接，rig 泛型不越过公开边界）。
/// backend 懒构建（首次 `embed` 时），使 `.with_base_url()` 在构造后仍生效。
/// 仅支持 `Text` 输入；`DataBlock` → `EmbeddingError::MultimodalNotSupported`
/// （契约 §4 能力矩阵：OpenAI embedding 模型无多模态）。
pub struct RigEmbeddingModel {
    model_name: String,
    api_key: String,
    base_url: Option<String>,
    backend: Mutex<Option<Arc<dyn RigEmbeddingBackend>>>,
    card: EmbeddingModelCard,
}

impl RigEmbeddingModel {
    /// OpenAI 后端（Embeddings API）。
    ///
    /// `api_key` 必填非空（空白 → `ValidationError`）；`ndims` 由 rig 按模型
    /// 标识查表。`base_url` 默认官方端点，用 `.with_base_url()` 覆盖。
    pub fn openai(
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ModelError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(ModelError::ValidationError {
                field: "api_key".to_string(),
                message: "api_key must not be empty".to_string(),
            });
        }
        let model = model.into();
        // 构建 backend 探知 ndims（rig build 仅解析 URL，无网络请求）。
        let backend = crate::openai::OpenAiEmbeddingBackend::new(&api_key, &model, None)?;
        let ndims = backend.ndims();
        let card = EmbeddingModelCard::new(model.clone(), ndims, false);
        Ok(Self {
            model_name: model,
            api_key,
            base_url: None,
            backend: Mutex::new(Some(Arc::new(backend))),
            card,
        })
    }

    /// 覆盖 base_url（backend 首次使用时校验合法 URL）。
    ///
    /// 更新配置并清空已构建 backend，使后续调用按新 URL 重建。
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        if let Ok(mut guard) = self.backend.lock() {
            *guard = None;
        }
        self
    }

    /// 取（或构建）embedding backend。
    fn backend(&self) -> Result<Arc<dyn RigEmbeddingBackend>, ModelError> {
        let mut guard = self
            .backend
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(b) = guard.as_ref() {
            return Ok(b.clone());
        }
        let built: Arc<dyn RigEmbeddingBackend> =
            Arc::new(crate::openai::OpenAiEmbeddingBackend::new(
                &self.api_key,
                &self.model_name,
                self.base_url.clone(),
            )?);
        *guard = Some(built.clone());
        Ok(built)
    }

    /// 测试注入构造器：直接放入预构建 backend（跳过懒构建）。
    ///
    /// `#[doc(hidden)]`：仅供 `tests/*.rs` 集成测试 mock `RigEmbeddingBackend`。
    #[doc(hidden)]
    pub fn from_backend_for_testing(
        model_name: impl Into<String>,
        ndims: u32,
        backend: Arc<dyn RigEmbeddingBackend>,
    ) -> Self {
        let model_name = model_name.into();
        let card = EmbeddingModelCard::new(model_name.clone(), ndims, false);
        Self {
            model_name,
            api_key: String::new(),
            base_url: None,
            backend: Mutex::new(Some(backend)),
            card,
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingModelTrait for RigEmbeddingModel {
    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError> {
        // 契约 §4：OpenAI embedding 不支持多模态；任一 DataBlock → 明确拒绝。
        if inputs
            .iter()
            .any(|i| matches!(i, EmbeddingInput::DataBlock(_)))
        {
            return Err(EmbeddingError::MultimodalNotSupported);
        }
        let texts: Vec<String> = inputs
            .into_iter()
            .filter_map(|i| match i {
                EmbeddingInput::Text(t) => Some(t),
                EmbeddingInput::DataBlock(_) => None,
            })
            .collect();
        let backend = self
            .backend()
            .map_err(|e| EmbeddingError::HttpError(e.to_string()))?;
        let embeddings = backend.embed_texts(texts).await?;
        Ok(EmbeddingResponse {
            embeddings,
            usage: EmbeddingUsage::default(),
        })
    }

    fn model_card(&self) -> &EmbeddingModelCard {
        &self.card
    }
}
