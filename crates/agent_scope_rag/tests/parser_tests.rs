//! Tests for the TextParser.
use agent_scope_rag::error::ParserError;
use agent_scope_rag::parser::{Parser, SectionContent, TextParser};

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
    let data = "content".as_bytes().to_vec();
    let result = parser.parse(data, "doc.pdf");
    assert!(result.is_err());
    assert!(matches!(
        result.expect_err("should error"),
        ParserError::UnsupportedFormat { .. }
    ));
}
