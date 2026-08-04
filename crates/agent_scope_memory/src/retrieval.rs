//! Model-based relevant memory selection — [`MemorySelection`] uses a [`ChatModel`] with
//! structured output to choose which memory files are relevant to a user query.

use std::sync::Arc;

use agent_scope_message::{Msg, factory::user_msg};
use agent_scope_model::ChatModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::index::truncate_text_to_tokens;
use crate::{FileMemory, Memory, MemoryError};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemorySelection {
    pub selected_files: Vec<String>,
}

pub async fn retrieve_relevant_files(
    memory: &FileMemory,
    query: &str,
    model: &Arc<dyn ChatModel>,
    max_results: usize,
) -> Result<Option<String>, MemoryError> {
    if query.trim().is_empty() {
        return Err(MemoryError::ValidationError {
            field: "query".into(),
            message: "query must not be empty".into(),
        });
    }

    // Use the untruncated enumeration: `list()` caps at `retrieval_max_files`
    // (200), so with more memory files the model would never see the older,
    // possibly relevant ones and would return nothing (round-4 M47; the search
    // and rebuild paths were already switched to `list_all_headers`).
    let headers = memory.list_all_headers().await?;
    if headers.is_empty() || max_results == 0 {
        return Ok(None);
    }

    let full_manifest = headers
        .iter()
        .map(|header| {
            format!(
                "- filename: {}\n  description: {}\n  type: {}",
                header.filename,
                header.description.as_deref().unwrap_or(""),
                header
                    .mem_type
                    .as_ref()
                    .map(|t| t.as_str())
                    .unwrap_or("unknown")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Cap the manifest so a very large memory store cannot exceed the model's
    // context window and silently fail the structured-output call (round-4 M47
    // follow-up). `truncate_text_to_tokens` returns the input unchanged when it
    // already fits the budget; otherwise it bisects to the longest prefix that
    // fits, and we then drop any entry it cut mid-line.
    let manifest_budget = ((model.context_size() / 4).max(2000)) as usize;
    let mut manifest = truncate_text_to_tokens(&full_manifest, manifest_budget, model.as_ref());
    if manifest != full_manifest {
        // `truncate_text_to_tokens` appends "<<<TRUNCATED>>>"; remove it and
        // any partial trailing entry (an entry always starts with "\n- ").
        if let Some(suffix) = manifest.rfind("\n<<<TRUNCATED>>>") {
            manifest.truncate(suffix);
        }
        if let Some(last_entry) = manifest.rfind("\n- ") {
            manifest.truncate(last_entry);
        }
    }

    let prompt = format!(
        "{}\n\nMemory manifest:\n{}\n\nUser query:\n{}\n\nReturn JSON with selected_files containing at most {} filenames.",
        memory.config.retrieval_instructions, manifest, query, max_results
    );
    let msg = user_msg("memory", &prompt).map_err(|err| MemoryError::RetrievalError {
        reason: format!("failed to build retrieval prompt: {err:?}"),
    })?;
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "selected_files": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["selected_files"]
    });

    let structured = match model.generate_structured_output(&[msg], &schema).await {
        Ok(response) => response,
        Err(err) => {
            warn!(error = %err, "memory retrieval model call failed");
            return Ok(None);
        }
    };

    let selection: MemorySelection = match serde_json::from_value(structured.content) {
        Ok(selection) => selection,
        Err(err) => {
            warn!(error = %err, "memory retrieval response parse failed");
            return Ok(None);
        }
    };

    let valid: std::collections::HashSet<&str> =
        headers.iter().map(|h| h.filename.as_str()).collect();
    let selected: Vec<String> = selection
        .selected_files
        .into_iter()
        .filter(|filename| valid.contains(filename.as_str()))
        .take(max_results)
        .collect();

    if selected.is_empty() {
        return Ok(None);
    }

    let mut sections = Vec::new();
    for filename in selected {
        let name = filename.trim_end_matches(".md");
        let Some(entry) = memory.read(name).await? else {
            continue;
        };
        let age = age_label(&entry.metadata.updated_at);
        let text = truncate_text_to_tokens(
            &entry.content,
            memory.config.retrieval_max_tokens_per_file,
            model.as_ref(),
        );
        sections.push(format!(
            "### {} ({})\nDescription: {}\nType: {}\n\n{}",
            entry.name,
            age,
            entry.description,
            entry.metadata.mem_type.as_str(),
            text
        ));
    }

    if sections.is_empty() {
        Ok(None)
    } else {
        Ok(Some(sections.join("\n\n")))
    }
}

fn age_label(updated_at: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(updated_at) else {
        return "saved at unknown time".into();
    };
    let now = chrono::Utc::now();
    let days = now
        .date_naive()
        .signed_duration_since(dt.with_timezone(&chrono::Utc).date_naive())
        .num_days();
    match days {
        0 => "saved today".into(),
        1 => "saved yesterday".into(),
        n if n > 1 => format!("saved {n} days ago"),
        _ => "saved today".into(),
    }
}

#[allow(dead_code)]
fn _assert_send_sync(_: &[Msg]) {}
