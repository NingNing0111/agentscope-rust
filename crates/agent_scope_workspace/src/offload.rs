//! Context and tool-result offloading.

use agent_scope_message::{ContentBlock, DataSource, Msg, ToolOutput, ToolResultBlock};

use crate::backend::WorkspaceBackend;
use crate::error::WorkspaceError;

pub async fn offload_context(
    session_id: &str,
    msgs: &[Msg],
    backend: &dyn WorkspaceBackend,
    sessions_dir: &str,
    data_dir: &str,
) -> Result<String, WorkspaceError> {
    let session_dir = backend.join_path(sessions_dir, &sanitize_component(session_id));
    if !backend.is_dir(&session_dir).await? {
        backend
            .write_file(&backend.join_path(&session_dir, ".keep"), b"")
            .await?;
    }

    if !backend.is_dir(data_dir).await? {
        backend
            .write_file(&backend.join_path(data_dir, ".keep"), b"")
            .await?;
    }

    let context_path = backend.join_path(&session_dir, "context.jsonl");

    let mut processed = Vec::with_capacity(msgs.len());
    for msg in msgs {
        let mut msg = msg.clone();
        for block in &mut msg.content {
            if let ContentBlock::Data(data_block) = block {
                handle_data_block_offload(data_block, backend, data_dir).await?;
            }
        }
        processed.push(msg);
    }

    let mut lines = String::new();
    for msg in &processed {
        let json = serde_json::to_string(msg).map_err(|e| WorkspaceError::OffloadError {
            message: format!("serialize Msg: {e}"),
        })?;
        lines.push_str(&json);
        lines.push('\n');
    }

    let mut existing = if backend.file_exists(&context_path).await? {
        String::from_utf8_lossy(&backend.read_file(&context_path).await?).to_string()
    } else {
        String::new()
    };
    existing.push_str(&lines);
    backend
        .write_file(&context_path, existing.as_bytes())
        .await?;

    Ok(context_path)
}

async fn handle_data_block_offload(
    data_block: &mut agent_scope_message::DataBlock,
    backend: &dyn WorkspaceBackend,
    data_dir: &str,
) -> Result<(), WorkspaceError> {
    if let DataSource::Base64(base64) = &data_block.source {
        let hash = hash_base64(&base64.data);
        let ext = mime_to_ext(&base64.media_type);
        let data_file = format!("{hash}.{ext}");
        let data_file_path = backend.join_path(data_dir, &data_file);

        if !backend.file_exists(&data_file_path).await? {
            let decoded =
                base64_decode(&base64.data).map_err(|e| WorkspaceError::OffloadError {
                    message: format!("base64 decode failed: {e}"),
                })?;
            backend.write_file(&data_file_path, &decoded).await?;
        }

        let file_url = format!("file://{data_file_path}");
        data_block.source = DataSource::Url(agent_scope_message::URLSource {
            url: file_url,
            media_type: base64.media_type.clone(),
        });
    }
    Ok(())
}

pub async fn offload_tool_result(
    session_id: &str,
    tool_result: &ToolResultBlock,
    backend: &dyn WorkspaceBackend,
    sessions_dir: &str,
    data_dir: &str,
) -> Result<String, WorkspaceError> {
    let session_dir = backend.join_path(sessions_dir, &sanitize_component(session_id));
    if !backend.is_dir(&session_dir).await? {
        backend
            .write_file(&backend.join_path(&session_dir, ".keep"), b"")
            .await?;
    }

    let safe_id = sanitize_component(&tool_result.id);
    let mut file_name = format!("tool_result-{safe_id}.txt");
    let mut file_path = backend.join_path(&session_dir, &file_name);

    let mut counter = 1;
    while backend.file_exists(&file_path).await? {
        file_name = format!("tool_result-{safe_id}-({counter}).txt");
        file_path = backend.join_path(&session_dir, &file_name);
        counter += 1;
    }

    let mut content = String::new();
    content.push_str(&format!("# Tool Result: {}\n", tool_result.name));
    content.push_str(&format!("ID: {}\n", tool_result.id));
    content.push_str(&format!("Created: {}\n\n", tool_result.created_at));

    match &tool_result.output {
        ToolOutput::Text(text) => {
            content.push_str("## Output\n\n");
            content.push_str(text);
        }
        ToolOutput::Blocks(blocks) => {
            for block in blocks {
                match block {
                    agent_scope_message::ToolResultBlockItem::Text(tb) => {
                        content.push_str(&tb.text);
                        content.push('\n');
                    }
                    agent_scope_message::ToolResultBlockItem::Data(db) => {
                        if let DataSource::Base64(base64) = &db.source {
                            let hash = hash_base64(&base64.data);
                            let ext = mime_to_ext(&base64.media_type);
                            let data_file = format!("{hash}.{ext}");
                            let data_file_path = backend.join_path(data_dir, &data_file);

                            if !backend.file_exists(&data_file_path).await? {
                                let decoded = base64_decode(&base64.data).map_err(|e| {
                                    WorkspaceError::OffloadError {
                                        message: format!("base64 decode: {e}"),
                                    }
                                })?;
                                backend.write_file(&data_file_path, &decoded).await?;
                            }
                            content.push_str(&format!(
                                "[data: {} (file://{data_file_path})]\n",
                                db.name.as_deref().unwrap_or("unnamed")
                            ));
                        } else if let DataSource::Url(url) = &db.source {
                            content.push_str(&format!(
                                "[data: {} ({})]\n",
                                db.name.as_deref().unwrap_or("unnamed"),
                                url.url
                            ));
                        }
                    }
                }
            }
        }
    }

    backend.write_file(&file_path, content.as_bytes()).await?;
    Ok(file_path)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hash_base64(data: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Reduce a value that is interpolated into a file path to a single safe path
/// component, so an untrusted `session_id` or tool-result `id` cannot smuggle
/// `..` or a path separator and escape the session/data directory.
///
/// A value made entirely of safe characters is returned unchanged (so existing
/// paths stay addressable). Only when a character must be replaced is a short
/// hash of the original appended, so two distinct inputs that sanitize to the
/// same component (e.g. `"a/b"` and `"a_b"`) do not silently collide (audit S7).
fn sanitize_component(value: &str) -> String {
    let mut needs_hash = false;
    let mut out = String::with_capacity(value.len() + 9);
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
            needs_hash = true;
        }
    }
    if out.is_empty() {
        out.push('_');
        needs_hash = true;
    }
    if needs_hash {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        let hash = hasher.finish();
        out.push('-');
        out.push_str(&format!("{hash:x}"));
    }
    out
}

fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())
}

fn mime_to_ext(media_type: &str) -> String {
    let ext: &str = match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "application/pdf" => "pdf",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        "video/mp4" => "mp4",
        "text/plain" => "txt",
        "text/html" => "html",
        "text/csv" => "csv",
        "application/json" => "json",
        "application/zip" => "zip",
        _ => "bin",
    };
    ext.to_string()
}
