//! DashScope Embedding Model — EmbeddingModel implementation for Alibaba Cloud DashScope.
//!
//! Calls the DashScope Text Embedding API at `/api/v1/services/embeddings/text-embedding/text-embedding`.

use std::sync::Arc;

use agent_scope_embedding::{
    EmbeddingCache, EmbeddingError, EmbeddingInput, EmbeddingModel, EmbeddingModelCard,
    EmbeddingResponse, EmbeddingUsage,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Request / Response types for DashScope Embedding API
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: EmbeddingRequestInput<'a>,
}

#[derive(Debug, Serialize)]
struct EmbeddingRequestInput<'a> {
    texts: &'a [String],
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponseBody {
    output: EmbeddingOutput,
    usage: Option<EmbeddingUsageBody>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingOutput {
    embeddings: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    #[serde(rename = "text_index")]
    _text_index: u32,
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingUsageBody {
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct EmbeddingErrorResponse {
    code: String,
    message: String,
}

// ---------------------------------------------------------------------------
// DashScopeEmbeddingModel
// ---------------------------------------------------------------------------

/// DashScope (Alibaba Cloud Model Studio) Embedding Model provider.
///
/// Communicates with the dashscope.aliyuncs.com Text Embedding API.
pub struct DashScopeEmbeddingModel {
    /// reqwest HTTP client.
    http_client: reqwest::Client,
    /// DashScope API key.
    api_key: String,
    /// Base URL for the embedding API.
    base_url: String,
    /// Model metadata card.
    model_card: EmbeddingModelCard,
    /// Optional embedding cache for response caching.
    cache: Option<Arc<dyn EmbeddingCache>>,
}

impl std::fmt::Debug for DashScopeEmbeddingModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashScopeEmbeddingModel")
            .field("base_url", &self.base_url)
            .field("model_card", &self.model_card)
            .finish_non_exhaustive()
    }
}

impl DashScopeEmbeddingModel {
    /// Create a new DashScope embedding model.
    ///
    /// # Arguments
    /// * `api_key` — DashScope API key
    /// * `model_card` — Model metadata (name, dimensions, supports_multimodal)
    pub fn new(api_key: String, model_card: EmbeddingModelCard) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            api_key,
            base_url: "https://dashscope.aliyuncs.com".to_string(),
            model_card,
            cache: None,
        }
    }

    /// Attach an embedding cache for response caching.
    pub fn with_cache(mut self, cache: Arc<dyn EmbeddingCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Set a custom base URL (for testing or regional endpoints).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[async_trait::async_trait]
impl EmbeddingModel for DashScopeEmbeddingModel {
    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError> {
        if self.api_key.is_empty() {
            return Err(EmbeddingError::ApiKeyMissing(
                "DASHSCOPE_API_KEY is not set".into(),
            ));
        }

        // Validate multimodal support
        if !self.supports_multimodal() {
            for input in &inputs {
                if matches!(input, EmbeddingInput::DataBlock(_)) {
                    return Err(EmbeddingError::MultimodalNotSupported);
                }
            }
        }

        // Extract text inputs (DataBlock is not yet implemented for DashScope)
        let texts: Vec<String> = inputs
            .iter()
            .map(|inp| match inp {
                EmbeddingInput::Text(s) => s.clone(),
                EmbeddingInput::DataBlock(_) => String::new(),
            })
            .collect();

        // Check cache if available
        if let Some(ref cache) = self.cache {
            let key = agent_scope_embedding::cache::hash_key(&texts.join("\x00"));
            if let Some(cached) = cache.lookup(&key) {
                return Ok(EmbeddingResponse {
                    embeddings: cached,
                    usage: EmbeddingUsage { total_tokens: 0 },
                });
            }
        }

        let url = format!(
            "{}/api/v1/services/embeddings/text-embedding/text-embedding",
            self.base_url
        );

        let request = EmbeddingRequest {
            model: &self.model_card.name,
            input: EmbeddingRequestInput { texts: &texts },
        };

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| EmbeddingError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();

            // Try to parse error response
            if let Ok(err) = serde_json::from_str::<EmbeddingErrorResponse>(&body) {
                return Err(EmbeddingError::ApiError {
                    code: err.code,
                    message: err.message,
                });
            }

            return Err(EmbeddingError::HttpError(format!("HTTP {status}: {body}")));
        }

        let body: EmbeddingResponseBody = response
            .json()
            .await
            .map_err(|e| EmbeddingError::DeserializationError(e.to_string()))?;

        // Extract embeddings sorted by text_index
        let mut items: Vec<(u32, Vec<f32>)> = body
            .output
            .embeddings
            .into_iter()
            .map(|item| (item._text_index, item.embedding))
            .collect();
        items.sort_by_key(|(idx, _)| *idx);

        let embeddings: Vec<Vec<f32>> = items.into_iter().map(|(_, emb)| emb).collect();

        // Validate dimension match
        let expected_dim = self.model_card.dimensions as usize;
        for (i, emb) in embeddings.iter().enumerate() {
            if emb.len() != expected_dim {
                return Err(EmbeddingError::DimensionMismatch {
                    expected: self.model_card.dimensions,
                    got: emb.len(),
                });
            }
            // Silence unused variable warning
            let _ = i;
        }

        let usage = EmbeddingUsage {
            total_tokens: body
                .usage
                .map(|u| u.total_tokens)
                .unwrap_or(texts.len() as u32),
        };

        let result = EmbeddingResponse { embeddings, usage };

        // Store in cache if available
        if let Some(ref cache) = self.cache {
            let key = agent_scope_embedding::cache::hash_key(&texts.join("\x00"));
            cache.store(&key, result.embeddings.clone());
        }

        Ok(result)
    }

    fn model_card(&self) -> &EmbeddingModelCard {
        &self.model_card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashscope_model_card() {
        let card = EmbeddingModelCard::new("text-embedding-v3", 1536, false);
        let model = DashScopeEmbeddingModel::new("test-key".into(), card);
        assert_eq!(model.model_card().name, "text-embedding-v3");
        assert_eq!(model.model_card().dimensions, 1536);
        assert!(!model.supports_multimodal());
    }

    #[test]
    fn test_dashscope_missing_api_key() {
        let card = EmbeddingModelCard::new("text-embedding-v3", 1024, false);
        let model = DashScopeEmbeddingModel::new(String::new(), card);

        let result = tokio_test::block_on(model.embed(vec!["hello".into()]));
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("should error"),
            EmbeddingError::ApiKeyMissing(_)
        ));
    }

    #[test]
    fn test_dashscope_datablock_rejected() {
        let card = EmbeddingModelCard::new("text-embedding-v3", 1024, false);
        let model = DashScopeEmbeddingModel::new("test-key".into(), card);

        let result =
            tokio_test::block_on(model.embed(vec![EmbeddingInput::DataBlock("image".into())]));
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("should error"),
            EmbeddingError::MultimodalNotSupported
        ));
    }
}
