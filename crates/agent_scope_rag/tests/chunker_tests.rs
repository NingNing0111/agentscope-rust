//! Tests for the ApproxTokenChunker.
use agent_scope_rag::chunker::{ApproxTokenChunker, Chunker};
use agent_scope_rag::error::ChunkerError;
use agent_scope_rag::parser::Section;

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

#[test]
fn test_chunker_500_words() {
    let chunker = ApproxTokenChunker::new(100, 20);
    let sections = vec![Section::new_text("doc.txt", generate_text(500))];
    let chunks = chunker.chunk(sections).expect("chunk should succeed");
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
fn test_chunker_empty_sections() {
    let chunker = ApproxTokenChunker::new(100, 20);
    let chunks = chunker.chunk(vec![]).expect("chunk should succeed");
    assert!(chunks.is_empty());
}

#[test]
fn test_chunker_empty_section_text() {
    let chunker = ApproxTokenChunker::new(100, 20);
    let section = Section::new_text("doc.txt", "");
    let chunks = chunker.chunk(vec![section]).expect("chunk should succeed");
    assert!(chunks.is_empty());
}

#[test]
fn test_chunker_invalid_params() {
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
fn test_chunker_cross_section_boundary() {
    let chunker = ApproxTokenChunker::new(100, 20);
    let sections = vec![
        Section::new_text("a.txt", generate_text(100)),
        Section::new_text("b.txt", generate_text(100)),
    ];
    let chunks = chunker.chunk(sections).expect("chunk should succeed");
    let a_chunks: Vec<_> = chunks.iter().filter(|c| c.source == "a.txt").collect();
    let b_chunks: Vec<_> = chunks.iter().filter(|c| c.source == "b.txt").collect();
    assert!(!a_chunks.is_empty());
    assert!(!b_chunks.is_empty());
}
