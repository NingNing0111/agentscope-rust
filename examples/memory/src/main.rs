//! Memory example: write and read a `FileMemory` (Markdown + YAML frontmatter).
//!
//! Needs no model or API key. Demonstrates the `Memory` trait: write entries,
//! list headers, read an entry back, and delete one.

use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let workdir = dir.path().to_str().unwrap();
    let config = MemoryConfig {
        memory_dir: "Memory".into(),
        ..MemoryConfig::default()
    };
    let memory = FileMemory::new(workdir, config, None);

    // 1. Write two memory entries.
    memory
        .write(MemoryEntry::new(
            "user_name",
            "The user's preferred name.",
            MemoryType::User,
            "Alice",
        ))
        .await?;
    memory
        .write(MemoryEntry::new(
            "user_preference",
            "A stated preference.",
            MemoryType::User,
            "Prefers concise answers.",
        ))
        .await?;
    println!("wrote 2 entries");

    // 2. List headers.
    let headers = memory.list().await?;
    println!("listed {} entries:", headers.len());
    for h in &headers {
        println!(
            "  - {} | {}",
            h.filename,
            h.description.as_deref().unwrap_or("")
        );
    }

    // 3. Read one entry back.
    if let Some(entry) = memory.read("user_name").await? {
        println!("read user_name → {}", entry.content);
    }

    // 4. Delete one entry.
    memory.delete("user_preference").await?;
    let headers = memory.list().await?;
    println!("after delete, {} entries remain", headers.len());

    assert_eq!(headers.len(), 1);
    println!("\nOK: FileMemory write/read/list/delete works.");
    Ok(())
}
