use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};
use agent_scope_message::Msg;
use agent_scope_model::{ChatModel, ChatResponse, ModelCallResult, ModelError, ToolChoice};
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
async fn index_tracks_writes_and_deletes() {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);
    for i in 0..5 {
        memory
            .write(MemoryEntry::new(
                format!("memory-{i}"),
                format!("Memory entry number {i}"),
                MemoryType::Project,
                format!("Content {i}"),
            ))
            .await
            .unwrap();
    }
    let index = memory.get_index_content().await.unwrap();
    assert_eq!(
        index.lines().filter(|line| line.starts_with("- [")).count(),
        5
    );

    memory.delete("memory-0").await.unwrap();
    let updated = memory.get_index_content().await.unwrap();
    assert_eq!(
        updated
            .lines()
            .filter(|line| line.starts_with("- ["))
            .count(),
        4
    );
    assert!(!updated.contains("memory-0"));
}

#[tokio::test]
async fn index_truncation_and_manual_edits_are_visible() {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);
    for i in 0..20 {
        memory
            .write(MemoryEntry::new(
                format!("m{i}"),
                "long description repeated repeated repeated",
                MemoryType::Project,
                "body",
            ))
            .await
            .unwrap();
    }
    let raw = memory.get_index_content().await.unwrap();
    let truncated = agent_scope_memory::truncate_index(&raw, 5, &TestModel);
    assert!(truncated.contains("<<<TRUNCATED"));

    tokio::fs::write(memory.index_path(), "- [manual](manual.md) — edited\n")
        .await
        .unwrap();
    assert!(memory.get_index_content().await.unwrap().contains("manual"));
}

#[tokio::test]
async fn index_generation_for_hundred_entries_is_fast() {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);
    let start = std::time::Instant::now();
    for i in 0..100 {
        memory
            .write(MemoryEntry::new(
                format!("m{i}"),
                format!("entry {i}"),
                MemoryType::Project,
                "body",
            ))
            .await
            .unwrap();
    }
    let elapsed = start.elapsed();
    // A 500ms cap is flaky under CI/io contention (atomic temp write + rename
    // per entry on slow machines); 2000ms still catches pathological regressions
    // without failing on loaded runners (round-5 L7).
    assert!(
        elapsed < std::time::Duration::from_millis(2000),
        "100 index writes took {elapsed:?} (expected < 2s)"
    );
}
