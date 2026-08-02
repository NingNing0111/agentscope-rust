//! `MEMORY.md` index file management — read, write, append, remove lines, and truncate
//! the memory index to stay within a configurable token budget.

use std::sync::Arc;

use agent_scope_message::{ContentBlock, Msg, TextBlock};
use agent_scope_model::ChatModel;

use crate::{Backend, FileMemory, MemoryError};

pub async fn read_index(backend: &dyn Backend, path: &str) -> Result<String, MemoryError> {
    if !backend.file_exists(path).await? {
        return Ok(String::new());
    }
    let bytes = backend.read_file(path).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub async fn write_index_line(
    backend: &dyn Backend,
    path: &str,
    filename: &str,
    description: &str,
) -> Result<(), MemoryError> {
    let current = read_index(backend, path).await?;
    let prefix = format!("- [{filename}](");
    let line = format!("- [{filename}]({filename}.md) — {description}");
    let mut replaced = false;
    let mut lines = Vec::new();
    for existing in current.lines() {
        if existing.starts_with(&prefix) {
            if !replaced {
                lines.push(line.clone());
                replaced = true;
            }
        } else {
            lines.push(existing.to_string());
        }
    }
    if !replaced {
        lines.push(line);
    }
    let mut output = lines.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    backend.write_file(path, output.as_bytes()).await
}

pub async fn remove_index_line(
    backend: &dyn Backend,
    path: &str,
    filename: &str,
) -> Result<(), MemoryError> {
    let current = read_index(backend, path).await?;
    let prefix = format!("- [{filename}](");
    let lines: Vec<&str> = current
        .lines()
        .filter(|line| !line.starts_with(&prefix))
        .collect();
    let mut output = lines.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    backend.write_file(path, output.as_bytes()).await
}

pub fn truncate_index(content: &str, max_tokens: usize, model: &dyn ChatModel) -> String {
    if content.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut kept = Vec::new();
    let mut tokens = 0usize;
    for line in &lines {
        let line_tokens = count_text_tokens(line, model);
        if tokens + line_tokens > max_tokens {
            break;
        }
        kept.push(*line);
        tokens += line_tokens;
    }
    if kept.len() == lines.len() {
        return content.to_string();
    }
    let omitted = lines.len().saturating_sub(kept.len());
    let mut result = kept.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result.push_str(&format!(
        "<<<TRUNCATED: {omitted} memory index lines omitted>>>"
    ));
    result
}

pub(crate) fn truncate_text_to_tokens(
    text: &str,
    max_tokens: usize,
    model: &dyn ChatModel,
) -> String {
    if count_text_tokens(text, model) <= max_tokens {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let mut candidate = out.clone();
        candidate.push(ch);
        if count_text_tokens(&candidate, model) > max_tokens {
            break;
        }
        out.push(ch);
    }
    out.push_str("\n<<<TRUNCATED>>>");
    out
}

fn count_text_tokens(text: &str, model: &dyn ChatModel) -> usize {
    let msg = Msg::new(
        "memory".into(),
        vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
        agent_scope_message::Role::User,
    )
    .unwrap_or_else(|_| {
        Msg::new(
            "memory".into(),
            Vec::new(),
            agent_scope_message::Role::Assistant,
        )
        .expect("assistant message permits empty content")
    });
    model.count_tokens(&[msg], None)
}

pub async fn read_index_for_memory(
    memory: &FileMemory,
    model: &Arc<dyn ChatModel>,
) -> Result<String, MemoryError> {
    let content = read_index(memory.backend.as_ref(), &memory.index_path()).await?;
    Ok(truncate_index(
        &content,
        memory.config.max_index_tokens,
        model.as_ref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalBackend;
    use agent_scope_model::{ChatResponse, ModelCallResult, ModelError, ToolChoice};
    use serde_json::Value as JsonValue;

    struct TestModel;

    #[async_trait::async_trait]
    impl ChatModel for TestModel {
        fn model_name(&self) -> &str {
            "test"
        }
        fn stream_enabled(&self) -> bool {
            false
        }
        async fn call_api(
            &self,
            _: &str,
            _: &[Msg],
            _: Option<&[JsonValue]>,
            _: Option<&ToolChoice>,
        ) -> Result<ModelCallResult, ModelError> {
            Ok(ModelCallResult::Complete(ChatResponse::default()))
        }
    }

    #[tokio::test]
    async fn writes_updates_and_deletes_index_lines() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalBackend::new();
        let path = dir.path().join("MEMORY.md").to_string_lossy().into_owned();
        write_index_line(&backend, &path, "a", "first")
            .await
            .unwrap();
        write_index_line(&backend, &path, "a", "second")
            .await
            .unwrap();
        let content = read_index(&backend, &path).await.unwrap();
        assert!(content.contains("second"));
        assert!(!content.contains("first"));
        remove_index_line(&backend, &path, "a").await.unwrap();
        assert!(!read_index(&backend, &path).await.unwrap().contains("a.md"));
    }

    #[tokio::test]
    async fn delete_nonexistent_line_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalBackend::new();
        let path = dir.path().join("MEMORY.md").to_string_lossy().into_owned();
        remove_index_line(&backend, &path, "missing").await.unwrap();
        assert_eq!(read_index(&backend, &path).await.unwrap(), "");
    }

    #[test]
    fn truncates_with_notice() {
        let model = TestModel;
        let content = "- [a](a.md) — a very long line\n- [b](b.md) — another long line\n";
        let truncated = truncate_index(content, 2, &model);
        assert!(truncated.contains("<<<TRUNCATED"));
    }

    #[test]
    fn empty_index_truncation_is_empty() {
        assert_eq!(truncate_index("", 10, &TestModel), "");
    }
}
