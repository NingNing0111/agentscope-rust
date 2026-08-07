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

/// When memory is disabled (state.memory is None), the tool must return a
/// clear "memory_disabled" error, not panic or silently succeed.
#[tokio::test]
async fn disabled_mode_returns_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let state = ToolState::new(dir.path().canonicalize().unwrap(), 1);
    assert!(state.memory.is_none(), "precondition: no memory configured");

    let result = memory_tool(
        &state,
        input("test-entry", "A test memory", "user", "Content here"),
    )
    .await;
    assert!(!result.ok, "memory tool must fail when disabled");
    let err = result.error.unwrap();
    assert_eq!(err.code, "memory_disabled");
    assert!(
        err.message.to_lowercase().contains("disabled"),
        "error message should mention disabled: {}",
        err.message
    );
}

/// When memory is disabled, writing an entry should not create any files on disk.
#[tokio::test]
async fn disabled_mode_writes_nothing_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let state = ToolState::new(dir.path().canonicalize().unwrap(), 1);
    let _ = memory_tool(
        &state,
        input("ghost-entry", "Should not appear", "user", "Ghost content"),
    )
    .await;
    // Verify no Memory directory or files were created.
    let mem_dir = dir.path().join("Memory");
    assert!(
        !mem_dir.exists(),
        "no Memory directory should be created in disabled mode"
    );
}

/// Corruption recovery: the name is sanitized; this test verifies that even
/// names with slashes or special chars produce a safe filename component.
#[tokio::test]
async fn sanitizes_dangerous_name_characters() {
    let dir = tempfile::tempdir().unwrap();
    let (state, memory_dir) = state_with_memory(&dir);
    // Names with path separators, colons, spaces, etc.
    let result = memory_tool(
        &state,
        input(
            "../../etc/passwd:evil",
            "Dangerous name test",
            "reference",
            "Should sanitize safely.",
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
    // Must not contain path separators or special chars.
    assert!(
        !stem.contains('/'),
        "filename must not contain slash: {stem}"
    );
    assert!(
        !stem.contains('\\'),
        "filename must not contain backslash: {stem}"
    );
    assert!(
        !stem.contains(".."),
        "filename must not contain '..': {stem}"
    );
    assert!(
        !stem.contains(':'),
        "filename must not contain colon: {stem}"
    );
    assert!(
        stem.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "filename must be a safe ASCII component: {stem}"
    );
}

/// Corruption recovery: if the MEMORY.md index is somehow corrupted (e.g.
/// truncated), a subsequent write should still succeed without panicking.
#[tokio::test]
async fn survives_corrupt_index_file() {
    let dir = tempfile::tempdir().unwrap();
    let (state, memory_dir) = state_with_memory(&dir);

    // Pre-write a corrupt (empty) MEMORY.md.
    std::fs::create_dir_all(&memory_dir).unwrap();
    std::fs::write(memory_dir.join("MEMORY.md"), [] as [u8; 0]).unwrap();

    // Writing a new entry should still succeed.
    let result = memory_tool(
        &state,
        input(
            "recovery-test",
            "Recover from corruption",
            "project",
            "Content",
        ),
    )
    .await;
    assert!(
        result.ok,
        "write should succeed despite corrupt index: {result:?}"
    );

    // The entry file should exist.
    let entry_path = memory_dir.join("recovery-test.md");
    assert!(entry_path.exists(), "entry file should exist");

    // The index should now contain the entry (recovered).
    let index = std::fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap();
    assert!(
        index.contains("Recover from corruption"),
        "index should show: {index}"
    );
}

/// Corruption recovery: an entry .md file that is empty or malformed should
/// not prevent a fresh write from succeeding (the write overwrites).
#[tokio::test]
async fn survives_corrupt_entry_file() {
    let dir = tempfile::tempdir().unwrap();
    let (state, memory_dir) = state_with_memory(&dir);

    // Pre-create a corrupt entry file.
    std::fs::create_dir_all(&memory_dir).unwrap();
    std::fs::write(memory_dir.join("broken-entry.md"), [0xff, 0xfe, 0xfd]).unwrap();

    // Overwriting via a write with the same name.
    let result = memory_tool(
        &state,
        input(
            "broken-entry",
            "Fixed entry",
            "project",
            "Repaired content.",
        ),
    )
    .await;
    assert!(
        result.ok,
        "write should succeed even if previous entry file was corrupt: {result:?}"
    );

    // The file should now contain valid content.
    let content = std::fs::read_to_string(memory_dir.join("broken-entry.md")).unwrap();
    assert!(
        content.contains("Repaired content"),
        "content should be repaired: {content}"
    );
}
