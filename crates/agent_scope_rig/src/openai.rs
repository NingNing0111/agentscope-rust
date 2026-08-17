//! OpenAI 后端（T016）——rig `CompletionsClient`（Chat Completions 协议）。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/provider-adapter.md` §2/§4
//! 与 `rig-mapping.md` §7。实现 [`RigChatBackend`]：`completion()`/`stream()`
//! 经 rig `CompletionRequest` 调用；rig 泛型类型在此边界内归一化为
//! [`NormCompletion`]/[`RigStreamDelta`]（宪法第十二条）。

use std::pin::Pin;

use futures::Stream;
use rig::client::{CompletionClient, EmbeddingsClient};
use rig::completion::{AssistantContent, CompletionModel, CompletionRequest};
use rig::providers::openai::CompletionsClient;

use agent_scope_embedding::error::EmbeddingError;
use agent_scope_model::model_error::ModelError;

use crate::backend::{
    NormCompletion, RigChatBackend, RigEmbeddingBackend, RigProviderCapabilities, RigStreamDelta,
};
use crate::error::map_completion_error;
use crate::stream::stream_to_delta_stream;

/// 构造 rig OpenAI Chat Completions client。
///
/// `api_key` 必填非空（空白 → `ValidationError`）；`base_url` 可选覆盖，
/// 提供时必须是合法 URL（`ValidationError`）。聊天与 embedding backend 共用。
fn build_completions_client(
    api_key: &str,
    base_url: Option<&str>,
) -> Result<CompletionsClient, ModelError> {
    if api_key.trim().is_empty() {
        return Err(ModelError::ValidationError {
            field: "api_key".to_string(),
            message: "api_key must not be empty".to_string(),
        });
    }

    let mut builder = CompletionsClient::builder().api_key(api_key);
    if let Some(url) = base_url {
        // 合法性校验：非法 URL → ValidationError（契约 §6 边界）。
        url::Url::parse(url).map_err(|e| ModelError::ValidationError {
            field: "base_url".to_string(),
            message: format!("invalid base_url for OpenAI: {e}"),
        })?;
        builder = builder.base_url(url);
    }
    builder.build().map_err(|e| ModelError::ValidationError {
        field: "base_url".to_string(),
        message: format!("failed to build rig OpenAI client: {e}"),
    })
}

/// OpenAI 后端：持有 rig Chat Completions client + 模型名 + 能力位。
///
/// 非泛型：`CompletionsClient` 固定为 `reqwest::Client` 传输（workspace 默认
/// feature），保证 `Send + Sync`（trait 对象要求）。
pub struct OpenAiBackend {
    client: CompletionsClient,
    model: String,
    capabilities: RigProviderCapabilities,
}

impl OpenAiBackend {
    /// 构造 OpenAI 后端。
    ///
    /// `api_key` 必填非空（空白 → `ValidationError`）；`base_url` 可选覆盖，
    /// 提供时必须是合法 URL（`ValidationError`）。
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
    ) -> Result<Self, ModelError> {
        let api_key = api_key.into();
        let model = model.into();
        let client = build_completions_client(&api_key, base_url.as_deref())?;

        Ok(Self {
            client,
            model,
            capabilities: RigProviderCapabilities {
                // OpenAI o 系列暴露 reasoning 内容；与 tool_choice=required 无互斥。
                supports_thinking: true,
                thinking_tool_choice_incompatible: false,
                supports_embedding: true,
            },
        })
    }
}

#[async_trait::async_trait]
impl RigChatBackend for OpenAiBackend {
    fn capabilities(&self) -> &RigProviderCapabilities {
        &self.capabilities
    }

    async fn completion(&self, request: CompletionRequest) -> Result<NormCompletion, ModelError> {
        let model = self.client.completion_model(self.model.clone());
        let response = model
            .completion(request)
            .await
            .map_err(|e| map_completion_error(&e, "openai"))?;
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
            .map_err(|e| map_completion_error(&e, "openai"))?;
        Ok(Box::pin(stream_to_delta_stream(stream, "openai")))
    }
}

/// OpenAI embedding 后端（T021）——rig `embedding_model`（Chat Completions client
/// 的 Embeddings capability）。
///
/// 实现 [`RigEmbeddingBackend`]：rig `EmbeddingModel` 的 `embed_texts`/`embed_text`
/// 在此边界内归一化为 `Vec<Vec<f32>>`（rig 向量是 `Vec<f64>`，按位转 `f32`）。
pub struct OpenAiEmbeddingBackend {
    client: CompletionsClient,
    model: String,
    ndims: u32,
}

impl OpenAiEmbeddingBackend {
    /// 构造 OpenAI embedding 后端。
    ///
    /// `api_key`/`base_url` 校验同 `build_completions_client`；`ndims` 由 rig 按
    /// 模型标识查表（`text-embedding-3-small`/`ada-002`→1536，`3-large`→3072）。
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
    ) -> Result<Self, ModelError> {
        let api_key = api_key.into();
        let model = model.into();
        let client = build_completions_client(&api_key, base_url.as_deref())?;
        let em = client.embedding_model(model.clone());
        let ndims = rig::embeddings::EmbeddingModel::ndims(&em) as u32;
        Ok(Self {
            client,
            model,
            ndims,
        })
    }
}

#[async_trait::async_trait]
impl RigEmbeddingBackend for OpenAiEmbeddingBackend {
    fn ndims(&self) -> u32 {
        self.ndims
    }

    async fn embed_texts(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        // 每次构建 embedding model 句柄（rig `make` 仅组装，无网络请求）。
        let em = self.client.embedding_model(self.model.clone());
        let embs = rig::embeddings::EmbeddingModel::embed_texts(&em, texts)
            .await
            .map_err(|e| EmbeddingError::HttpError(e.to_string()))?;
        Ok(embs
            .into_iter()
            .map(|e| e.vec.into_iter().map(|v| v as f32).collect())
            .collect())
    }

    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let em = self.client.embedding_model(self.model.clone());
        let e = rig::embeddings::EmbeddingModel::embed_text(&em, text)
            .await
            .map_err(|e| EmbeddingError::HttpError(e.to_string()))?;
        Ok(e.vec.into_iter().map(|v| v as f32).collect())
    }
}
