//! xberg-backed parser for PDF, Office, Excel, and HTML.

use std::collections::HashMap;
use std::future::Future;

use xberg::{ExtractInput, ExtractedDocument, ExtractionConfig, PageConfig, extract};

use crate::error::ParserError;
use crate::parser::{Parser, Section, SectionContent};

/// Parser for PDF / DOCX / PPTX / XLSX / HTML via xberg.
///
/// Scanned-image PDFs are out of scope: OCR is not enabled.
pub struct XbergParser;

impl XbergParser {
    fn extension(filename: &str) -> String {
        std::path::Path::new(filename)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn mime_for_filename(filename: &str) -> Option<&'static str> {
        match Self::extension(filename).as_str() {
            "pdf" => Some("application/pdf"),
            "docx" => {
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
            }
            "pptx" => {
                Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
            }
            "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            "html" | "htm" => Some("text/html"),
            _ => None,
        }
    }

    fn unsupported(filename: &str) -> ParserError {
        ParserError::UnsupportedFormat {
            format: Self::extension(filename),
            filename: filename.to_string(),
        }
    }

    fn extraction_error(filename: &str, error: impl ToString) -> ParserError {
        ParserError::ExtractionError {
            filename: filename.to_string(),
            error: error.to_string(),
        }
    }

    async fn extract_sections(
        file: Vec<u8>,
        filename: String,
    ) -> Result<Vec<Section>, ParserError> {
        if file.is_empty() {
            return Ok(vec![]);
        }
        let mime =
            Self::mime_for_filename(&filename).ok_or_else(|| Self::unsupported(&filename))?;
        let config = ExtractionConfig {
            use_cache: false,
            pages: Some(PageConfig {
                extract_pages: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let input = ExtractInput::from_bytes(file, mime, Some(filename.clone()));
        let output = extract(input, &config)
            .await
            .map_err(|e| Self::extraction_error(&filename, e))?;
        // `errors` are non-fatal per-input items; prefer a successful document
        // when both are present, and only fail when `results` is empty.
        let Some(doc) = output.results.into_iter().next() else {
            return Err(match output.errors.first() {
                Some(err) => Self::extraction_error(&filename, &err.message),
                None => Self::extraction_error(&filename, "no extraction result"),
            });
        };
        Ok(sections_from_document(doc, &filename))
    }

    fn parse_blocking(&self, file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError> {
        let filename = filename.to_string();
        let run = |file: Vec<u8>, filename: String| {
            // Drive extract on a private runtime. Reusing `Handle::block_on` on a
            // `current_thread` parent deadlocks: that runtime has one worker, and
            // it is stuck in `thread::scope` waiting for this call to finish.
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| Self::extraction_error(&filename, e))?
                .block_on(Self::extract_sections(file, filename))
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::scope(|scope| {
                scope
                    .spawn(|| run(file, filename.clone()))
                    .join()
                    .unwrap_or_else(|_| {
                        Err(Self::extraction_error(
                            &filename,
                            "extraction thread panicked",
                        ))
                    })
            })
        } else {
            run(file, filename)
        }
    }
}

impl Parser for XbergParser {
    fn parse(&self, file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError> {
        self.parse_blocking(file, filename)
    }

    fn parse_async(
        &self,
        file: Vec<u8>,
        filename: &str,
    ) -> impl Future<Output = Result<Vec<Section>, ParserError>> + Send {
        Self::extract_sections(file, filename.to_string())
    }
}

fn sections_from_document(doc: ExtractedDocument, filename: &str) -> Vec<Section> {
    if let Some(pages) = doc.pages.filter(|pages| !pages.is_empty()) {
        let mut sections = Vec::new();
        for page in pages {
            let mut text = page.content;
            append_table_markdown(&mut text, page.tables.iter().map(|t| t.markdown.as_str()));
            if text.trim().is_empty() {
                continue;
            }
            let mut metadata = HashMap::new();
            metadata.insert("page".into(), page.page_number.to_string());
            if let Some(sheet) = page.sheet_name {
                metadata.insert("sheet".into(), sheet);
            }
            sections.push(Section {
                content: SectionContent::Text(text),
                source: filename.to_string(),
                metadata,
            });
        }
        if !sections.is_empty() {
            return sections;
        }
    }

    let mut text = doc.content;
    append_table_markdown(&mut text, doc.tables.iter().map(|t| t.markdown.as_str()));
    if text.trim().is_empty() {
        return vec![];
    }
    vec![Section::new_text(filename, text)]
}

fn append_table_markdown<'a>(text: &mut String, tables: impl IntoIterator<Item = &'a str>) {
    for markdown in tables {
        if markdown.trim().is_empty() || text.contains(markdown) {
            continue;
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push('\n');
        text.push_str(markdown);
    }
}
