//! Error types for the memory system — [`MemoryError`] covers I/O, parsing, validation,
//! indexing, retrieval, and semantic-index failures.

use std::fmt;

#[derive(Debug)]
pub enum MemoryError {
    IoError { path: String, message: String },
    ParseError { filename: String, message: String },
    ValidationError { field: String, message: String },
    NotFound { name: String },
    IndexError { message: String },
    RetrievalError { reason: String },
    SemanticIndexError { reason: String },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError { path, message } => write!(f, "I/O error at '{path}': {message}"),
            Self::ParseError { filename, message } => {
                write!(f, "Failed to parse memory file '{filename}': {message}")
            }
            Self::ValidationError { field, message } => {
                write!(f, "Invalid memory field '{field}': {message}")
            }
            Self::NotFound { name } => write!(f, "Memory '{name}' was not found"),
            Self::IndexError { message } => write!(f, "Memory index error: {message}"),
            Self::RetrievalError { reason } => write!(f, "Memory retrieval error: {reason}"),
            Self::SemanticIndexError { reason } => {
                write!(f, "Semantic index error: {reason}")
            }
        }
    }
}

impl std::error::Error for MemoryError {}
