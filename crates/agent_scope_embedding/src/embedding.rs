//! Embedding model trait and associated types.
//!
//! Provides the [`EmbeddingModel`] trait for text/multimodal embedding,
//! and supporting data types: [`EmbeddingInput`], [`EmbeddingResponse`],
//! [`EmbeddingUsage`], and [`EmbeddingModelCard`].

use serde::{Deserialize, Serialize};

use crate::error::EmbeddingError;

// ---------------------------------------------------------------------------
// EmbeddingInput
// ---------------------------------------------------------------------------

/// Input to an embedding model.
///
/// Text inputs are always supported. `DataBlock` inputs require the model
/// to set `supports_multimodal = true` in its [`EmbeddingModelCard`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmbeddingInput {
    /// Plain text input.
    Text(String),
    /// Multi-modal input (images, audio, etc.).
    /// The model MUST have `supports_multimodal = true` to accept this variant.
    DataBlock(String),
}

impl From<String> for EmbeddingInput {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for EmbeddingInput {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// EmbeddingUsage
// ---------------------------------------------------------------------------

/// Token usage for an embedding request.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    /// Total tokens consumed.
    pub total_tokens: u32,
}

// ---------------------------------------------------------------------------
// EmbeddingResponse
// ---------------------------------------------------------------------------

/// Embedding result from a model call.
///
/// # Invariants
///
/// - `embeddings.len()` MUST equal the number of inputs.
/// - Each `embeddings[i].len()` MUST equal `model_card().dimensions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// One vector per input, each of length `model_card().dimensions`.
    pub embeddings: Vec<Vec<f32>>,
    /// Token usage statistics.
    pub usage: EmbeddingUsage,
}

// ---------------------------------------------------------------------------
// EmbeddingModelCard
// ---------------------------------------------------------------------------

/// Static metadata describing an embedding model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelCard {
    /// Model identifier (e.g., "text-embedding-v3").
    pub name: String,
    /// Output vector dimensionality.
    pub dimensions: u32,
    /// Whether this model supports multi-modal inputs (images, etc.).
    pub supports_multimodal: bool,
}

impl EmbeddingModelCard {
    /// Create a new model card.
    pub fn new(name: impl Into<String>, dimensions: u32, supports_multimodal: bool) -> Self {
        Self {
            name: name.into(),
            dimensions,
            supports_multimodal,
        }
    }
}

// ---------------------------------------------------------------------------
// EmbeddingModel trait
// ---------------------------------------------------------------------------

/// Trait for embedding model providers.
///
/// Semantically equivalent to Python AgentScope `EmbeddingModelBase.__call__`.
///
/// # Contract
///
/// - `embed()` returns one vector per input, each of length `model_card().dimensions`
/// - `model_card()` returns the same metadata for the lifetime of the model
/// - `supports_multimodal()` returns `model_card().supports_multimodal` by default
/// - When `supports_multimodal() == false` and inputs include `DataBlock`,
///   implementations must return `EmbeddingError::MultimodalNotSupported`.
#[async_trait::async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// Convert inputs to dense vectors.
    async fn embed(&self, inputs: Vec<EmbeddingInput>)
    -> Result<EmbeddingResponse, EmbeddingError>;

    /// Return static model metadata.
    fn model_card(&self) -> &EmbeddingModelCard;

    /// Whether this model handles multi-modal (DataBlock) inputs.
    fn supports_multimodal(&self) -> bool {
        self.model_card().supports_multimodal
    }
}
