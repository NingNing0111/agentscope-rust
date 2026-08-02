//! AgentScope RAG System — Parser, Chunker, VectorStore, KnowledgeBase,
//! and RAGMiddleware for agent knowledge integration.

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]

pub mod chunker;
pub mod error;
pub mod knowledge_base;
pub mod parser;
pub mod rag_middleware;
pub mod turbovec_memory_adapter;
pub mod turbovec_store;
pub mod vector_store;

// Re-exports — chunker
pub use chunker::{ApproxTokenChunker, Chunk, Chunker};

// Re-exports — error types
pub use error::{ChunkerError, KnowledgeBaseError, ParserError, VectorStoreError};

// Re-exports — knowledge base
pub use knowledge_base::KnowledgeBase;

// Re-exports — parser
pub use parser::{Parser, Section, SectionContent, TextParser};

// Re-exports — RAG middleware
pub use rag_middleware::{RAGMiddleware, RAGMode};

// Re-exports — TurboVec integration
pub use turbovec_memory_adapter::TurbovecIndexAdapter;
pub use turbovec_store::{CalibrationState, TurbovecVectorStore};

// Re-exports — vector store
pub use vector_store::{DocumentSummary, VectorRecord, VectorSearchResult, VectorStore};
