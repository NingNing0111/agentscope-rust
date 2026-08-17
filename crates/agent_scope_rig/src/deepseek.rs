//! DeepSeek 后端——rig `deepseek::Client`（OpenAI-compatible 协议）。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/provider-adapter.md` §2/§4
//! 与 `rig-mapping.md` §7。实现 [`RigChatBackend`]：`completion()`/`stream()`
//! 经 rig `CompletionRequest` 调用；rig 泛型类型在此边界内归一化为
//! [`NormCompletion`]/[`RigStreamDelta`]（宪法第十二条）。
//!
//! DeepSeek 复用 OpenAI-compatible Chat Completions 协议，rig 的
//! `CompletionClient` 泛型 impl（`Capabilities<H, Completion=Capable<M>>`）
//! 与 OpenAI 同一路径；`StreamingUsage` 为 `deepseek::Usage`（impl
//! `GetTokenUsage`），共享转换器 `stream_to_delta_stream` 按 trait 归一化。

use std::pin::Pin;

use futures::Stream;
use rig::client::CompletionClient;
use rig::completion::{AssistantContent, CompletionModel, CompletionRequest};
use rig::providers::deepseek::Client;

use agent_scope_model::model_error::ModelError;

use crate::backend::{NormCompletion, RigChatBackend, RigProviderCapabilities, RigStreamDelta};
use crate::error::map_completion_error;
use crate::stream::stream_to_delta_stream;

/// DeepSeek 后端：持有 rig client + 模型名 + 能力位。
///
/// 非泛型：`Client` 固定为 `reqwest::Client` 传输（workspace 默认 feature），
/// 保证 `Send + Sync`（trait 对象要求）。
pub struct DeepSeekBackend {
    client: Client,
    model: String,
    capabilities: RigProviderCapabilities,
}

impl DeepSeekBackend {
    /// 构造 DeepSeek 后端。
    ///
    /// `api_key` 必填非空（空白 → `ValidationError`）；`base_url` 可选覆盖，
    /// 提供时必须是合法 URL（`ValidationError`）。
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
                message: format!("invalid base_url for DeepSeek: {e}"),
            })?;
            builder = builder.base_url(url);
        }
        let client = builder.build().map_err(|e| ModelError::ValidationError {
            field: "base_url".to_string(),
            message: format!("failed to build rig DeepSeek client: {e}"),
        })?;

        Ok(Self {
            client,
            model,
            capabilities: RigProviderCapabilities {
                // DeepSeek 推理模型暴露 reasoning 内容；与 tool_choice 无互斥。
                supports_thinking: true,
                thinking_tool_choice_incompatible: false,
                // DeepSeek 官方 API 无 embeddings 端点。
                supports_embedding: false,
            },
        })
    }
}

#[async_trait::async_trait]
impl RigChatBackend for DeepSeekBackend {
    fn capabilities(&self) -> &RigProviderCapabilities {
        &self.capabilities
    }

    async fn completion(&self, request: CompletionRequest) -> Result<NormCompletion, ModelError> {
        let model = self.client.completion_model(self.model.clone());
        let response = model
            .completion(request)
            .await
            .map_err(|e| map_completion_error(&e, "deepseek"))?;
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
            .map_err(|e| map_completion_error(&e, "deepseek"))?;
        // rig 流式响应 → RigStreamDelta 增量流（共享转换器，stream.rs）。
        Ok(Box::pin(stream_to_delta_stream(stream, "deepseek")))
    }
}
