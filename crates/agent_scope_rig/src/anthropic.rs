//! Anthropic 后端——rig `anthropic::Client`（Messages API 协议）。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/provider-adapter.md` §2/§4
//! 与 `rig-mapping.md` §7。实现 [`RigChatBackend`]：`completion()`/`stream()`
//! 经 rig `CompletionRequest` 调用；rig 泛型类型在此边界内归一化为
//! [`NormCompletion`]/[`RigStreamDelta`]（宪法第十二条）。
//!
//! 能力位：`thinking_tool_choice_incompatible = true`——Anthropic reasoning
//! 与 `tool_choice` 同时指定会 400（契约 US4 降级守卫的保守设置）。

use std::pin::Pin;

use futures::Stream;
use rig::client::CompletionClient;
use rig::completion::{AssistantContent, CompletionModel, CompletionRequest};
use rig::providers::anthropic::Client;

use agent_scope_model::model_error::ModelError;

use crate::backend::{NormCompletion, RigChatBackend, RigProviderCapabilities, RigStreamDelta};
use crate::error::map_completion_error;
use crate::stream::stream_to_delta_stream;

/// Anthropic 后端：持有 rig Messages client + 模型名 + 能力位。
///
/// 非泛型：`Client` 固定为 `reqwest::Client` 传输（workspace 默认 feature），
/// 保证 `Send + Sync`（trait 对象要求）。
pub struct AnthropicBackend {
    client: Client,
    model: String,
    capabilities: RigProviderCapabilities,
}

impl AnthropicBackend {
    /// 构造 Anthropic 后端。
    ///
    /// `api_key` 必填非空（空白 → `ValidationError`）；`base_url` 可选覆盖，
    /// 提供时必须是合法 URL（`ValidationError`）。Anthropic 的 `max_tokens`
    /// 必需字段由 rig `default_max_tokens` 兜底（request 缺省时）。
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
    ) -> Result<Self, ModelError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(ModelError::ValidationError {
                field: "api_key".to_string(),
                message: "api_key must not be empty".to_string(),
            });
        }
        let model = model.into();

        let mut builder = Client::builder().api_key(api_key);
        if let Some(url) = base_url.as_ref() {
            url::Url::parse(url).map_err(|e| ModelError::ValidationError {
                field: "base_url".to_string(),
                message: format!("invalid base_url for Anthropic: {e}"),
            })?;
            builder = builder.base_url(url);
        }
        let client = builder.build().map_err(|e| ModelError::ValidationError {
            field: "base_url".to_string(),
            message: format!("failed to build rig Anthropic client: {e}"),
        })?;

        Ok(Self {
            client,
            model,
            capabilities: RigProviderCapabilities {
                // Anthropic 支持 reasoning 内容；但 reasoning 与 tool_choice 互斥
                // （同请求指定会 400）——置 true 触发引擎侧降级守卫（契约 US4）。
                supports_thinking: true,
                thinking_tool_choice_incompatible: true,
                // Anthropic Messages API 无 embeddings 端点。
                supports_embedding: false,
            },
        })
    }
}

#[async_trait::async_trait]
impl RigChatBackend for AnthropicBackend {
    fn capabilities(&self) -> &RigProviderCapabilities {
        &self.capabilities
    }

    async fn completion(&self, request: CompletionRequest) -> Result<NormCompletion, ModelError> {
        let model = self.client.completion_model(self.model.clone());
        let response = model
            .completion(request)
            .await
            .map_err(|e| map_completion_error(&e, "anthropic"))?;
        let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
        Ok(NormCompletion {
            choice,
            usage: response.usage,
            message_id: response.message_id,
        })
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<RigStreamDelta, ModelError>> + Send>>, ModelError>
    {
        let model = self.client.completion_model(self.model.clone());
        let stream = model
            .stream(request)
            .await
            .map_err(|e| map_completion_error(&e, "anthropic"))?;
        // rig 流式响应 → RigStreamDelta 增量流（共享转换器，stream.rs）。
        Ok(Box::pin(stream_to_delta_stream(stream, "anthropic")))
    }
}
