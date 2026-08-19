//! Document parser trait and text parser implementation.
//!
//! Parsers convert raw file bytes into logical [`Section`] units that
//! can then be fed to a [`Chunker`](crate::chunker::Chunker).

use std::collections::HashMap;
use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::error::ParserError;

// ---------------------------------------------------------------------------
// SectionContent
// ---------------------------------------------------------------------------

/// The content of a document section — either plain text or a data block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SectionContent {
    /// Plain text content.
    Text(String),
    /// Multi-modal data block (images, audio, etc.).
    DataBlock(String),
}

// ---------------------------------------------------------------------------
// Section
// ---------------------------------------------------------------------------

/// A logical boundary unit within a source document.
///
/// Produced by [`Parser`], consumed by [`Chunker`](crate::chunker::Chunker).
///
/// # Invariants
///
/// - Different sections are never merged across by the chunker
/// - Empty files produce an empty `Vec` (no sections)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section content (text or data block).
    pub content: SectionContent,
    /// Source filename identifier.
    pub source: String,
    /// Format-specific metadata (page number, slide index, sheet name, etc.).
    pub metadata: HashMap<String, String>,
}

impl Section {
    /// Create a new text section.
    pub fn new_text(source: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            content: SectionContent::Text(text.into()),
            source: source.into(),
            metadata: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Parser trait
// ---------------------------------------------------------------------------

/// Trait for document parsers.
///
/// Takes raw bytes + filename and returns logical sections.
///
/// # Implementations
///
/// - [`TextParser`] — handles `.txt` and `.md` files
/// - `XbergParser` (feature `xberg`) — PDF / Office / Excel / HTML
pub trait Parser: Send + Sync {
    /// Parse raw file content into sections.
    ///
    /// # Arguments
    /// * `file` — raw file content as bytes
    /// * `filename` — original filename (for source attribution and format detection)
    fn parse(&self, file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError>;

    /// Async parse entry used by ingest. Defaults to the synchronous [`Parser::parse`].
    fn parse_async(
        &self,
        file: Vec<u8>,
        filename: &str,
    ) -> impl Future<Output = Result<Vec<Section>, ParserError>> + Send {
        let result = self.parse(file, filename);
        async move { result }
    }
}

// ---------------------------------------------------------------------------
// TextParser
// ---------------------------------------------------------------------------

/// Parser for plain text files (`.txt`, `.md`).
///
/// Converts the entire file content to a UTF-8 string and wraps it
/// as a single [`Section`]. Empty files produce an empty `Vec`.
///
/// In v1, Markdown files are treated as plain text (no heading-based splitting).
/// This matches the Python AgentScope reference behavior.
pub struct TextParser;

impl Parser for TextParser {
    fn parse(&self, file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError> {
        // Empty file → empty sections
        if file.is_empty() {
            return Ok(vec![]);
        }

        // Validate format by extension
        let lower = filename.to_lowercase();
        let is_supported = lower.ends_with(".txt")
            || lower.ends_with(".md")
            || lower.ends_with(".markdown")
            || lower.ends_with(".text");

        if !is_supported {
            let ext = std::path::Path::new(filename)
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return Err(ParserError::UnsupportedFormat {
                format: ext,
                filename: filename.to_string(),
            });
        }

        // Decode UTF-8
        let text = String::from_utf8(file).map_err(|e| ParserError::EncodingError {
            filename: filename.to_string(),
            error: e.to_string(),
        })?;

        Ok(vec![Section::new_text(filename, text)])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_parser_basic_txt() {
        let parser = TextParser;
        let data = "Hello, world!".as_bytes().to_vec();
        let sections = parser
            .parse(data, "test.txt")
            .expect("parse should succeed");

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].source, "test.txt");
        if let SectionContent::Text(ref t) = sections[0].content {
            assert_eq!(t, "Hello, world!");
        } else {
            panic!("expected Text content");
        }
    }

    #[test]
    fn test_text_parser_markdown() {
        let parser = TextParser;
        let data = "# Title\n\nContent".as_bytes().to_vec();
        let sections = parser.parse(data, "doc.md").expect("parse should succeed");

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].source, "doc.md");
    }

    #[test]
    fn test_text_parser_empty_file() {
        let parser = TextParser;
        let sections = parser
            .parse(vec![], "empty.txt")
            .expect("parse should succeed");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_text_parser_utf8_error() {
        let parser = TextParser;
        // Invalid UTF-8: 0xFF is never valid in UTF-8
        let data = vec![0xff, 0xfe, 0xfd];
        let result = parser.parse(data, "bad.txt");
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("should error"),
            ParserError::EncodingError { .. }
        ));
    }

    #[test]
    fn test_text_parser_unsupported_format() {
        let parser = TextParser;
        let data = "binary content".as_bytes().to_vec();
        let result = parser.parse(data, "doc.pdf");
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("should error"),
            ParserError::UnsupportedFormat { .. }
        ));
    }

    #[test]
    fn test_text_parser_empty_filename_unsupported() {
        let parser = TextParser;
        let data = "some content".as_bytes().to_vec();
        // No extension → not recognized
        let result = parser.parse(data, "file_without_extension");
        assert!(result.is_err());
    }
}
