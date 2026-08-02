# Quickstart: TurboVec Long-Term Memory

**Feature**: 022-turbovec-long-term-memory | **Date**: 2026-08-02

## Prerequisites

- Rust toolchain (stable)
- 64-bit Linux or macOS (turbovec requires `target_pointer_width = "64"`)
- No external database or service required

## Scenario 1: Create and Search Memories

```rust
use agent_scope_memory::{Memory, MemoryEntry, MemoryType, TurbovecMemory, TurbovecMemoryConfig};
use agent_scope_embedding::{EmbeddingModel, MockEmbeddingModel}; // or DashScopeEmbeddingModel

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use a mock embedding model (1536-dim deterministic vectors) for tests,
    // or a real model for production.
    let embedding = Arc::new(MockEmbeddingModel::new(1536));

    let config = TurbovecMemoryConfig {
        memory_dir: "/tmp/turbovec-memory".into(),
        ..Default::default()
    };

    let memory = TurbovecMemory::new("/tmp/turbovec-memory", config, embedding, None).await?;

    // Write memories
    memory.write(MemoryEntry::new(
        "user-role",
        "User is a data scientist",
        MemoryType::User,
        "The user works as a senior data scientist focusing on NLP and recommendation systems."
    )).await?;

    memory.write(MemoryEntry::new(
        "project-deploy",
        "Deployment preferences",
        MemoryType::Project,
        "The user prefers deploying to Kubernetes with Helm charts. Staging first, then production."
    )).await?;

    // Semantic search (via TurboVec)
    let results = memory.semantic_search("kubernetes deployment", Some(MemoryType::Project), 5).await?;
    for r in &results {
        println!("[{:.4}] {}: {}", r.score, r.memory_name, r.description);
    }
    // Expected: "project-deploy" ranks highest

    Ok(())
}
```

**Expected outcome**: `project-deploy` returned with highest score; `user-role` scored lower or excluded.

## Scenario 2: Persist and Reload

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let embedding = Arc::new(MockEmbeddingModel::new(1536));
    let dir = "/tmp/turbovec-memory-persist";

    // First session: write and persist
    {
        let config = TurbovecMemoryConfig { memory_dir: dir.into(), ..Default::default() };
        let memory = TurbovecMemory::new(dir, config, embedding.clone(), None).await?;
        memory.write(MemoryEntry::new("note-1", "test note", MemoryType::Reference, "important content")).await?;
        memory.save_index().await?;  // persist vector index to .turbovec/
    }

    // Second session: reload
    {
        let config = TurbovecMemoryConfig { memory_dir: dir.into(), ..Default::default() };
        let memory = TurbovecMemory::new(dir, config, embedding.clone(), None).await?;
        // Load existing index; if not found and auto_rebuild is true, rebuild from files
        let results = memory.semantic_search("important content", None, 5).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory_name, "note-1");
    }

    Ok(())
}
```

**Expected outcome**: Reloaded instance returns same search results without re-insertion.

## Scenario 3: Type-Filtered Retrieval

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let embedding = Arc::new(MockEmbeddingModel::new(1536));
    let config = TurbovecMemoryConfig::default();
    let memory = TurbovecMemory::new("/tmp/turbovec-memory-types", config, embedding, None).await?;

    // Mix of types
    memory.write(MemoryEntry::new("u1", "user name", MemoryType::User, "Alice")).await?;
    memory.write(MemoryEntry::new("p1", "project lang", MemoryType::Project, "Rust")).await?;
    memory.write(MemoryEntry::new("r1", "rust book", MemoryType::Reference, "The Rust Book")).await?;

    let user_results = memory.semantic_search("who is the user", Some(MemoryType::User), 3).await?;
    // user_results should contain "u1" but not "p1" or "r1"

    let project_results = memory.semantic_search("language", Some(MemoryType::Project), 3).await?;
    // project_results should contain "p1" but not user/reference entries

    Ok(())
}
```

**Expected outcome**: Type filter restricts results to matching category only.

## Scenario 4: Rebuild Index

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let embedding = Arc::new(MockEmbeddingModel::new(1536));
    let dir = "/tmp/turbovec-memory-rebuild";
    let config = TurbovecMemoryConfig { memory_dir: dir.into(), auto_rebuild: true, ..Default::default() };

    let memory = TurbovecMemory::new(dir, config, embedding.clone(), None).await?;

    // Write memories
    memory.write(MemoryEntry::new("a", "entry a", MemoryType::User, "content a")).await?;
    memory.write(MemoryEntry::new("b", "entry b", MemoryType::Project, "content b")).await?;

    // Simulate index loss (delete .turbovec/)
    std::fs::remove_dir_all(format!("{dir}/.turbovec"))?;

    // Search triggers auto-rebuild because auto_rebuild=true
    let results = memory.semantic_search("content a", None, 5).await?;
    assert!(!results.is_empty());

    // Or explicitly rebuild and inspect report
    let report = memory.rebuild_index().await?;
    println!("Rebuilt: {} indexed, {} skipped, {} errors in {}ms",
        report.indexed, report.skipped, report.errors.len(), report.duration_ms);

    Ok(())
}
```

**Expected outcome**: Search works after auto-rebuild; explicit rebuild returns a valid report.

## Run Tests

```bash
# TurboVec memory unit tests
cargo test -p agent_scope_memory turbovec

# Semantic search tests
cargo test -p agent_scope_memory semantic -- --nocapture

# Integration: memory + embedding + vector store
cargo test -p agent_scope_memory --test turbovec_memory_tests

# Clippy and format
cargo clippy -p agent_scope_memory -- -D warnings
cargo fmt -p agent_scope_memory -- --check
```

## Platform Note

turbovec requires 64-bit target (`x86_64` or `aarch64`). WASM and 32-bit targets are not supported. CI jobs targeting unsupported platforms should cfg-gate or skip turbovec-dependent tests.
