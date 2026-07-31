//! AgentScope Embedding Model API — EmbeddingModel trait, EmbeddingCache,
//! and provider-independent abstractions.

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]

pub mod cache;
pub mod embedding;
pub mod error;

// Re-exports
pub use cache::{EmbeddingCache, FileEmbeddingCache};
pub use embedding::{
    EmbeddingInput, EmbeddingModel, EmbeddingModelCard, EmbeddingResponse, EmbeddingUsage,
};
pub use error::EmbeddingError;
