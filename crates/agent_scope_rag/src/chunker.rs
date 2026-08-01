//! Text chunker trait and approximate-token-count implementation.
//!
//! Chunkers split [`Section`](crate::parser::Section) lists into
//! search-indexable [`Chunk`] fragments.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::ChunkerError;
use crate::parser::{Section, SectionContent};

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

/// An indexable text fragment produced by a [`Chunker`].
///
/// # Invariants
///
/// - All chunks from the same document have the same `total_chunks` value
/// - `chunk_index` counts from 0 monotonically
/// - Chunks may overlap (controlled by the `overlap` parameter)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// The chunk's text content.
    pub content: String,
    /// Source filename (inherited from Section).
    pub source: String,
    /// Zero-based position within the document.
    pub chunk_index: usize,
    /// Total number of chunks from the same document.
    pub total_chunks: usize,
    /// Additional metadata (inherited + chunker-specific).
    pub metadata: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Chunker trait
// ---------------------------------------------------------------------------

/// Trait for text chunkers.
///
/// Consumes [`Section`]s and produces [`Chunk`]s.
///
/// # Contract
///
/// - Different sections are never merged into the same chunk
/// - Empty section lists produce empty chunk lists (not an error)
/// - `total_chunks` is set per-document (same source)
pub trait Chunker: Send + Sync {
    /// Split sections into indexable chunks.
    fn chunk(&self, sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>;
}

// ---------------------------------------------------------------------------
// ApproxTokenChunker
// ---------------------------------------------------------------------------

/// Approximate token-count based chunker.
///
/// Uses heuristic token counting:
/// - English text: whitespace-split words (1 word ≈ 1 token)
/// - CJK-like text: characters / 4 ≈ tokens
///
/// This approximates the Python AgentScope behavior without requiring
/// a full tokenizer library.
#[derive(Debug, Clone)]
pub struct ApproxTokenChunker {
    /// Target tokens per chunk.
    pub chunk_size: usize,
    /// Overlap tokens between adjacent chunks.
    pub overlap: usize,
}

impl ApproxTokenChunker {
    /// Create a new chunker with the given parameters.
    ///
    /// # Validation
    /// `chunk_size` must be greater than `overlap`.
    pub fn new(chunk_size: usize, overlap: usize) -> Self {
        Self {
            chunk_size,
            overlap,
        }
    }

    /// Validate parameters. Returns `Ok(())` or `Err(...)`.
    fn validate(&self) -> Result<(), ChunkerError> {
        if self.chunk_size <= self.overlap {
            return Err(ChunkerError::InvalidParameters {
                chunk_size: self.chunk_size,
                overlap: self.overlap,
            });
        }
        Ok(())
    }

    /// Estimate token count for a string.
    ///
    /// - English (ASCII letters/numbers with spaces): word count
    /// - Non-ASCII, non-whitespace (CJK, etc.): chars / 4
    #[allow(dead_code)]
    fn estimate_tokens(&self, text: &str) -> usize {
        let mut tokens = 0_usize;
        let mut current_word: Option<usize> = None;

        for ch in text.chars() {
            if ch.is_whitespace() {
                // End current word
                if let Some(count) = current_word.take()
                    && count > 0
                {
                    tokens += 1; // 1 word ≈ 1 token
                }
            } else if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
                // ASCII word building
                current_word.get_or_insert(0);
                if let Some(ref mut c) = current_word {
                    *c += 1;
                }
            } else {
                // Non-ASCII (CJK etc.) — approximate as 4 chars ≈ 1 token
                // Flush any current ASCII word first
                if let Some(count) = current_word.take()
                    && count > 0
                {
                    tokens += 1;
                }
                tokens += 1; // 1 CJK char ≈ 0.25 tokens, round up to at least 1
            }
        }

        // Flush last word
        if let Some(count) = current_word
            && count > 0
        {
            tokens += 1;
        }

        // CJK adjustment: count CJK characters and divide by 4
        let cjk_count = text
            .chars()
            .filter(|ch| !ch.is_whitespace() && !ch.is_ascii_alphabetic() && !ch.is_ascii_digit())
            .count();
        // We already counted 1 per CJK char above, now correct to chars/4
        // Remove the 1-for-1 count and replace with chars/4
        if cjk_count > 0 {
            tokens = tokens - cjk_count + (cjk_count / 4).max(1);
        }

        tokens.max(1)
    }

    /// Split a text string into chunks using sliding window.
    fn chunk_text(&self, text: &str, source: &str, base_index: usize) -> Vec<Chunk> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return vec![];
        }

        let stride = self.chunk_size - self.overlap;
        let mut chunks = Vec::new();
        let mut pos = 0_usize;

        while pos < words.len() {
            let end = (pos + self.chunk_size).min(words.len());
            let window: Vec<&str> = words[pos..end].to_vec();
            let content = window.join(" ");

            chunks.push(Chunk {
                content,
                source: source.to_string(),
                chunk_index: base_index + chunks.len(),
                total_chunks: 0, // filled in later
                metadata: HashMap::new(),
            });

            if end >= words.len() {
                break;
            }
            pos += stride;
            if pos >= words.len() {
                break;
            }
        }

        // Fill in total_chunks
        let total = chunks.len();
        for chunk in &mut chunks {
            chunk.total_chunks = total;
        }

        chunks
    }
}

impl Chunker for ApproxTokenChunker {
    fn chunk(&self, sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError> {
        self.validate()?;

        if sections.is_empty() {
            return Ok(vec![]);
        }

        let mut all_chunks: Vec<Chunk> = Vec::new();

        // Track global index per-source
        let mut source_index: HashMap<String, usize> = HashMap::new();

        for section in sections {
            let index_offset = source_index.get(&section.source).copied().unwrap_or(0);

            match &section.content {
                SectionContent::Text(text) => {
                    if text.trim().is_empty() {
                        continue;
                    }
                    let chunks = self.chunk_text(text, &section.source, index_offset);
                    let count = chunks.len();
                    source_index.insert(section.source.clone(), index_offset + count);
                    all_chunks.extend(chunks);
                }
                SectionContent::DataBlock(_data) => {
                    // DataBlock sections: produce a single chunk with placeholder content
                    let chunk = Chunk {
                        content: String::from("[DataBlock]"),
                        source: section.source.clone(),
                        chunk_index: index_offset,
                        total_chunks: 1,
                        metadata: section.metadata.clone(),
                    };
                    source_index.insert(section.source.clone(), index_offset + 1);
                    all_chunks.push(chunk);
                }
            }
        }

        // Recalculate total_chunks per source
        let mut source_totals: HashMap<String, usize> = HashMap::new();
        for chunk in &all_chunks {
            let entry = source_totals.entry(chunk.source.clone()).or_insert(0);
            *entry += 1;
        }
        for chunk in &mut all_chunks {
            chunk.total_chunks = source_totals.get(&chunk.source).copied().unwrap_or(1);
        }

        Ok(all_chunks)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approx_token_chunker_basic() {
        let chunker = ApproxTokenChunker::new(100, 20);
        let sections = vec![Section::new_text("doc.txt", generate_text(500))];
        let chunks = chunker.chunk(sections).expect("chunk should succeed");

        // 500 words / (100-20=80 stride) ≈ 5-7 chunks
        assert!(
            chunks.len() >= 5,
            "expected at least 5 chunks, got {}",
            chunks.len()
        );

        for chunk in &chunks {
            assert_eq!(chunk.source, "doc.txt");
            assert_eq!(chunk.total_chunks, chunks.len());
        }
    }

    #[test]
    fn test_approx_token_chunker_empty_sections() {
        let chunker = ApproxTokenChunker::new(100, 20);
        let chunks = chunker.chunk(vec![]).expect("chunk should succeed");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_approx_token_chunker_empty_section_text() {
        let chunker = ApproxTokenChunker::new(100, 20);
        let section = Section::new_text("empty.txt", "");
        let chunks = chunker.chunk(vec![section]).expect("chunk should succeed");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_approx_token_chunker_invalid_params() {
        // overlap >= chunk_size
        let chunker = ApproxTokenChunker::new(100, 100);
        let sections = vec![Section::new_text("doc.txt", "hello world")];
        let result = chunker.chunk(sections);
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("should error"),
            ChunkerError::InvalidParameters { .. }
        ));
    }

    #[test]
    fn test_approx_token_chunker_cross_section_boundary() {
        let chunker = ApproxTokenChunker::new(100, 20);
        let sections = vec![
            Section::new_text("a.txt", generate_text(100)),
            Section::new_text("b.txt", generate_text(100)),
        ];
        let chunks = chunker.chunk(sections).expect("chunk should succeed");

        // Sections from different sources should not be merged
        let a_chunks: Vec<_> = chunks.iter().filter(|c| c.source == "a.txt").collect();
        let b_chunks: Vec<_> = chunks.iter().filter(|c| c.source == "b.txt").collect();
        assert!(!a_chunks.is_empty());
        assert!(!b_chunks.is_empty());
    }

    #[test]
    fn test_estimate_tokens_english() {
        let chunker = ApproxTokenChunker::new(100, 20);
        let tokens = chunker.estimate_tokens("hello world test");
        // 3 words ≈ 3 tokens
        assert!((2..=4).contains(&tokens));
    }

    /// Generate `n` words of lorem-ipsum-like text.
    fn generate_text(n: usize) -> String {
        let words = [
            "lorem",
            "ipsum",
            "dolor",
            "sit",
            "amet",
            "consectetur",
            "adipiscing",
            "elit",
            "sed",
            "do",
            "eiusmod",
            "tempor",
            "incididunt",
            "ut",
            "labore",
            "et",
            "dolore",
            "magna",
            "aliqua",
            "enim",
            "ad",
            "minim",
            "veniam",
            "quis",
            "nostrud",
            "exercitation",
            "ullamco",
            "laboris",
            "nisi",
            "aliquip",
        ];
        (0..n)
            .map(|i| words[i % words.len()])
            .collect::<Vec<_>>()
            .join(" ")
    }
}
