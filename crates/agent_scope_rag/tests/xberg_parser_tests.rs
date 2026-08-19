//! Tests for XbergParser (feature = "xberg").
#![cfg(feature = "xberg")]

use agent_scope_rag::XbergParser;
use agent_scope_rag::error::ParserError;
use agent_scope_rag::parser::{Parser, SectionContent};

fn fixture(name: &str) -> &'static [u8] {
    match name {
        "hello.html" => include_bytes!("fixtures/hello.html"),
        "hello.pdf" => include_bytes!("fixtures/hello.pdf"),
        "hello.docx" => include_bytes!("fixtures/hello.docx"),
        "hello.xlsx" => include_bytes!("fixtures/hello.xlsx"),
        other => panic!("unknown fixture {other}"),
    }
}

fn text_of(sections: &[agent_scope_rag::Section]) -> String {
    sections
        .iter()
        .map(|s| match &s.content {
            SectionContent::Text(t) => t.as_str(),
            SectionContent::DataBlock(t) => t.as_str(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn test_xberg_parser_html() {
    let sections = XbergParser
        .parse_async(fixture("hello.html").to_vec(), "hello.html")
        .await
        .expect("html parse");
    assert!(!sections.is_empty());
    assert!(text_of(&sections).contains("Hello RAG from HTML"));
}

#[tokio::test]
async fn test_xberg_parser_pdf() {
    let sections = XbergParser
        .parse_async(fixture("hello.pdf").to_vec(), "hello.pdf")
        .await
        .expect("pdf parse");
    assert!(!sections.is_empty());
    assert!(
        text_of(&sections).contains("Hello RAG from PDF"),
        "got: {}",
        text_of(&sections)
    );
}

#[tokio::test]
async fn test_xberg_parser_docx() {
    let sections = XbergParser
        .parse_async(fixture("hello.docx").to_vec(), "hello.docx")
        .await
        .expect("docx parse");
    assert!(!sections.is_empty());
    assert!(
        text_of(&sections).contains("Hello RAG from DOCX"),
        "got: {}",
        text_of(&sections)
    );
}

#[tokio::test]
async fn test_xberg_parser_xlsx() {
    let sections = XbergParser
        .parse_async(fixture("hello.xlsx").to_vec(), "hello.xlsx")
        .await
        .expect("xlsx parse");
    assert!(!sections.is_empty());
    let joined = text_of(&sections);
    assert!(joined.contains("Hello RAG from XLSX"), "got: {joined}");
}

#[tokio::test]
async fn test_xberg_parser_empty_file() {
    let sections = XbergParser
        .parse_async(vec![], "empty.pdf")
        .await
        .expect("empty parse");
    assert!(sections.is_empty());
}

#[tokio::test]
async fn test_xberg_parser_unsupported() {
    let err = XbergParser
        .parse_async(b"not-an-image".to_vec(), "scan.png")
        .await
        .expect_err("png should be unsupported without OCR");
    assert!(matches!(err, ParserError::UnsupportedFormat { .. }));
}

#[test]
fn test_xberg_parser_sync_html_without_runtime() {
    let sections = XbergParser
        .parse(fixture("hello.html").to_vec(), "hello.html")
        .expect("sync html parse");
    assert!(text_of(&sections).contains("Hello RAG from HTML"));
}

#[tokio::test]
async fn test_xberg_parser_sync_from_current_thread_runtime() {
    let sections = XbergParser
        .parse(fixture("hello.html").to_vec(), "hello.html")
        .expect("sync parse under tokio::test current_thread");
    assert!(text_of(&sections).contains("Hello RAG from HTML"));
}

#[tokio::test]
async fn test_xberg_parser_htm_and_uppercase_extension() {
    for name in ["hello.htm", "HELLO.HTML"] {
        let sections = XbergParser
            .parse_async(fixture("hello.html").to_vec(), name)
            .await
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            text_of(&sections).contains("Hello RAG from HTML"),
            "{name}: {}",
            text_of(&sections)
        );
    }
}

#[tokio::test]
async fn test_xberg_parser_corrupt_pdf() {
    let err = XbergParser
        .parse_async(b"not-a-pdf".to_vec(), "bad.pdf")
        .await
        .expect_err("corrupt pdf should fail extraction");
    assert!(matches!(err, ParserError::ExtractionError { .. }));
}
