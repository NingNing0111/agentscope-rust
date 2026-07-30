use std::sync::Arc;

use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryError, MemoryType};

fn make_entry(name: &str, mem_type: MemoryType, content: &str) -> MemoryEntry {
    MemoryEntry::new(name, format!("Description for {name}"), mem_type, content)
}

#[tokio::test]
async fn file_memory_crud_all_types_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);

    for (name, mem_type) in [
        ("user", MemoryType::User),
        ("feedback", MemoryType::Feedback),
        ("project", MemoryType::Project),
        ("reference", MemoryType::Reference),
    ] {
        memory
            .write(make_entry(
                name,
                mem_type.clone(),
                &format!("{name} logging content"),
            ))
            .await
            .unwrap();
        let read = memory.read(name).await.unwrap().unwrap();
        assert_eq!(read.metadata.mem_type, mem_type);
    }

    memory
        .write(MemoryEntry::new(
            "user",
            "Updated user",
            MemoryType::User,
            "updated",
        ))
        .await
        .unwrap();
    assert_eq!(
        memory.read("user").await.unwrap().unwrap().content,
        "updated"
    );

    let results = memory.search("logging", None).await.unwrap();
    assert!(results.iter().any(|entry| entry.name == "feedback"));
    let filtered = memory
        .search("content", Some(MemoryType::Reference))
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "reference");

    let headers = memory.list().await.unwrap();
    assert!(headers.iter().all(|header| !header.filename.is_empty()));

    memory.delete("user").await.unwrap();
    assert!(memory.read("user").await.unwrap().is_none());
    memory.delete("user").await.unwrap();
}

#[tokio::test]
async fn empty_directory_returns_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);
    assert!(memory.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn validates_memory_entry_edges() {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);

    let err = memory
        .write(MemoryEntry::new("", "desc", MemoryType::User, ""))
        .await
        .unwrap_err();
    assert!(matches!(err, MemoryError::ValidationError { field, .. } if field == "name"));

    let err = memory
        .write(MemoryEntry::new("valid", "", MemoryType::User, ""))
        .await
        .unwrap_err();
    assert!(matches!(err, MemoryError::ValidationError { field, .. } if field == "description"));

    let err = memory
        .write(MemoryEntry::new("bad/name", "desc", MemoryType::User, ""))
        .await
        .unwrap_err();
    assert!(matches!(err, MemoryError::ValidationError { field, .. } if field == "name"));

    let long_content = "x".repeat(20_000);
    memory
        .write(MemoryEntry::new(
            "long",
            "desc",
            MemoryType::Project,
            long_content.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(
        memory.read("long").await.unwrap().unwrap().content,
        long_content
    );
}

#[tokio::test]
async fn trait_object_works() {
    let dir = tempfile::tempdir().unwrap();
    let memory: Arc<dyn Memory> = Arc::new(FileMemory::new(
        dir.path().to_str().unwrap(),
        MemoryConfig::default(),
        None,
    ));
    memory
        .write(MemoryEntry::new(
            "trait",
            "trait desc",
            MemoryType::Project,
            "body",
        ))
        .await
        .unwrap();
    assert!(memory.read("trait").await.unwrap().is_some());
}
