//! Error types for the RAG system.
//!
//! Each subsystem defines its own error type per Constitution §XIII.

use std::fmt;

// ---------------------------------------------------------------------------
// ParserError
// ---------------------------------------------------------------------------

/// Errors from document parsing.
#[derive(Debug, Clone)]
pub enum ParserError {
    /// File format not supported by this parser.
    UnsupportedFormat {
        /// The file extension or format name.
        format: String,
        /// The original filename.
        filename: String,
    },
    /// UTF-8 decoding failed.
    EncodingError {
        /// The original filename.
        filename: String,
        /// Description of the encoding error.
        error: String,
    },
    /// Format-specific extraction failed.
    ExtractionError {
        /// The original filename.
        filename: String,
        /// Description of the extraction error.
        error: String,
    },
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat { format, filename } => {
                write!(f, "unsupported format '{format}' for file '{filename}'")
            }
            Self::EncodingError { filename, error } => {
                write!(f, "encoding error in '{filename}': {error}")
            }
            Self::ExtractionError { filename, error } => {
                write!(f, "extraction error in '{filename}': {error}")
            }
        }
    }
}

impl std::error::Error for ParserError {}

// ---------------------------------------------------------------------------
// ChunkerError
// ---------------------------------------------------------------------------

/// Errors from text chunking.
#[derive(Debug, Clone)]
pub enum ChunkerError {
    /// Invalid parameters (e.g., chunk_size <= overlap).
    InvalidParameters {
        /// The configured chunk size.
        chunk_size: usize,
        /// The configured overlap.
        overlap: usize,
    },
}

impl fmt::Display for ChunkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters {
                chunk_size,
                overlap,
            } => {
                write!(
                    f,
                    "invalid chunker parameters: chunk_size ({chunk_size}) must be > overlap ({overlap})"
                )
            }
        }
    }
}

impl std::error::Error for ChunkerError {}

// ---------------------------------------------------------------------------
// VectorStoreError
// ---------------------------------------------------------------------------

/// Errors from vector store operations.
#[derive(Debug, Clone)]
pub enum VectorStoreError {
    /// The collection does not exist.
    CollectionNotFound(String),
    /// The collection already exists.
    CollectionAlreadyExists(String),
    /// Dimension mismatch.
    DimensionMismatch {
        /// Expected dimension.
        expected: u32,
        /// Actual dimension received.
        got: usize,
    },
    /// Backend-specific error.
    BackendError(String),
    /// Operation timed out.
    Timeout(String),
}

impl fmt::Display for VectorStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CollectionNotFound(name) => write!(f, "collection '{name}' not found"),
            Self::CollectionAlreadyExists(name) => {
                write!(f, "collection '{name}' already exists")
            }
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
            Self::BackendError(msg) => write!(f, "backend error: {msg}"),
            Self::Timeout(msg) => write!(f, "operation timed out: {msg}"),
        }
    }
}

impl std::error::Error for VectorStoreError {}

// ---------------------------------------------------------------------------
// KnowledgeBaseError
// ---------------------------------------------------------------------------

/// Errors from knowledge base operations.
#[derive(Debug, Clone)]
pub enum KnowledgeBaseError {
    /// Embedding model returned an error.
    EmbeddingError(String),
    /// Vector store returned an error.
    VectorStoreError(String),
    /// Number of embeddings doesn't match number of chunks.
    CountMismatch {
        /// Expected count (number of chunks).
        expected: usize,
        /// Actual count (number of embeddings received).
        got: usize,
    },
    /// Dimension mismatch (embedding vs collection).
    DimensionMismatch {
        /// Expected dimension.
        expected: u32,
        /// Actual dimension.
        got: u32,
    },
}

impl fmt::Display for KnowledgeBaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmbeddingError(msg) => write!(f, "embedding error: {msg}"),
            Self::VectorStoreError(msg) => write!(f, "vector store error: {msg}"),
            Self::CountMismatch { expected, got } => {
                write!(
                    f,
                    "count mismatch: expected {expected} embeddings, got {got}"
                )
            }
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for KnowledgeBaseError {}

// ---------------------------------------------------------------------------
// IngestError
// ---------------------------------------------------------------------------

/// Errors from the parse → chunk → insert pipeline.
#[derive(Debug, Clone)]
pub enum IngestError {
    /// Document parsing failed.
    Parser(ParserError),
    /// Text chunking failed.
    Chunker(ChunkerError),
    /// Knowledge base insert failed.
    KnowledgeBase(KnowledgeBaseError),
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parser(e) => write!(f, "ingest parse error: {e}"),
            Self::Chunker(e) => write!(f, "ingest chunk error: {e}"),
            Self::KnowledgeBase(e) => write!(f, "ingest knowledge base error: {e}"),
        }
    }
}

impl std::error::Error for IngestError {}

impl From<ParserError> for IngestError {
    fn from(value: ParserError) -> Self {
        Self::Parser(value)
    }
}

impl From<ChunkerError> for IngestError {
    fn from(value: ChunkerError) -> Self {
        Self::Chunker(value)
    }
}

impl From<KnowledgeBaseError> for IngestError {
    fn from(value: KnowledgeBaseError) -> Self {
        Self::KnowledgeBase(value)
    }
}
