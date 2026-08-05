//! Tests for the pi-rust Memory tool — the long-term memory *write* path.
//!
//! The library's `MemoryMiddleware` only reads (index injection + retrieval);
//! persisting facts is the Memory tool's job, writing to `workdir/Memory/*.md`
//! plus the MEMORY.md index line.

use std::path::PathBuf;
use std::sync::Arc;

use agent_scope_memory::{FileMemory, Memory, MemoryConfig};
use pi_rust::tools::{MemoryInput, ToolState, memory_tool};

fn state_with_memory(dir: &tempfile::TempDir) -> (ToolState, PathBuf) {
    let memory_dir = dir.path().join("Memory");
    let config = MemoryConfig {
        memory_dir: memory_dir.to_string_lossy().to_string(),
        ..MemoryConfig::default()
    };
    let memory: Arc<dyn Memory> =
        Arc::new(FileMemory::new(dir.path().to_str().unwrap(), config, None));
    let mut state = ToolState::new(dir.path().canonicalize().unwrap(), 1);
    state.memory = Some(memory);
    (state, memory_dir)
}

fn input(name: &str, description: &str, mem_type: &str, content: &str) -> MemoryInput {
    MemoryInput {
        name: name.into(),
        description: description.into(),
        mem_type: mem_type.into(),
        content: content.into(),
    }
}

#[tokio::test]
async fn writes_entry_and_index() {
    let dir = tempfile::tempdir().unwrap();
    let (state, memory_dir) = state_with_memory(&dir);
    let result = memory_tool(
        &state,
        input(
            "user-greeting",
            "The user's greeting is Hello Rust",
            "user",
            "The user always greets with 'Hello Rust'.",
        ),
    )
    .await;
    assert!(result.ok, "{result:?}");
    assert!(
        dir.path().join("Memory/user-greeting.md").exists(),
        "entry file missing"
    );
    let index = std::fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap();
    assert!(index.contains("Hello Rust"), "{index}");
    assert!(index.contains("user-greeting"), "{index}");
}

#[tokio::test]
async fn sanitizes_chinese_name() {
    let dir = tempfile::tempdir().unwrap();
    let (state, memory_dir) = state_with_memory(&dir);
    let result = memory_tool(
        &state,
        input(
            "用户-张德帅",
            "用户叫张德帅",
            "user",
            "用户的名字是张德帅。",
        ),
    )
    .await;
    assert!(result.ok, "{result:?}");
    let entries: Vec<String> = std::fs::read_dir(&memory_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".md") && name != "MEMORY.md")
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let stem = entries[0].trim_end_matches(".md");
    assert!(
        stem.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "filename must be a safe ASCII component: {}",
        entries[0]
    );
    // The semantic detail lives in the index description, which may be CJK.
    let index = std::fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap();
    assert!(index.contains("用户叫张德帅"), "{index}");
}

#[tokio::test]
async fn roundtrip_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    let (state, memory_dir) = state_with_memory(&dir);
    let result = memory_tool(
        &state,
        input(
            "user-name",
            "The user's name is 张德帅",
            "user",
            "The user is called 张德帅.",
        ),
    )
    .await;
    assert!(result.ok, "{result:?}");

    // Simulate a restart: a fresh FileMemory over the same memory_dir must see
    // the entry that the previous process wrote.
    let config = MemoryConfig {
        memory_dir: memory_dir.to_string_lossy().to_string(),
        ..MemoryConfig::default()
    };
    let reloaded = FileMemory::new(dir.path().to_str().unwrap(), config, None);
    let entry = reloaded
        .read("user-name")
        .await
        .unwrap()
        .expect("entry should survive across instances");
    assert!(entry.content.contains("张德帅"));
}

#[tokio::test]
async fn disabled_without_memory() {
    let dir = tempfile::tempdir().unwrap();
    let state = ToolState::new(dir.path().canonicalize().unwrap(), 1);
    let result = memory_tool(&state, input("x", "d", "user", "c")).await;
    assert!(!result.ok);
    assert_eq!(result.error.unwrap().code, "memory_disabled");
}

#[tokio::test]
async fn rejects_unknown_type() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _) = state_with_memory(&dir);
    let result = memory_tool(&state, input("x", "d", "bogus", "c")).await;
    assert!(!result.ok);
    assert_eq!(result.error.unwrap().code, "invalid_arguments");
}

#[tokio::test]
async fn rejects_empty_description() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _) = state_with_memory(&dir);
    let result = memory_tool(&state, input("x", "  ", "user", "c")).await;
    assert!(!result.ok);
    assert_eq!(result.error.unwrap().code, "invalid_arguments");
}

#[tokio::test]
async fn rejects_empty_name() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _) = state_with_memory(&dir);
    let result = memory_tool(&state, input("   ", "d", "user", "c")).await;
    assert!(!result.ok);
    assert_eq!(result.error.unwrap().code, "invalid_arguments");
}
