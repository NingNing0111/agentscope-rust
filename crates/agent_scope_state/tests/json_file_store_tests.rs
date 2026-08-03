//! Integration tests for JsonFileSessionStore.
//!
//! Covers the on-disk JSON session store added in Feature 025
//! (agent state persistence with a built-in JSON file backend):
//!
//! - Round-trip save/load preserving the full AgentState (quickstart 场景 1)
//! - Atomic writes, corrupted-file errors, invalid session-id rejection (quickstart 场景 6)
//! - Lightweight list_meta / idempotent delete (quickstart 场景 8)
//!
//! See `specs/025-agent-state-persistence/` for the design contracts:
//! `data-model.md`, `contracts/session-store.md`, `contracts/json-file-format.md`.

use std::collections::HashMap;
use std::fs;

use agent_scope_message::{ContentBlock, Role, TextBlock};
use agent_scope_state::{
    JsonFileSessionStore, Session, SessionError, SessionImpl, SessionStore, SummaryContent, Task,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// T008 — round-trip save/load (quickstart 场景 1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_save_load_round_trip_preserves_full_state() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());

    let mut session = SessionImpl::with_session_id("s1".into());
    // Dialogue context: several messages.
    for i in 0..5 {
        let blocks = vec![ContentBlock::Text(TextBlock::new(format!("msg-{i}")))];
        session.state_mut().append_context("agent", blocks).unwrap();
    }
    // Summary.
    session.state_mut().summary = SummaryContent::Text("summarized".into());
    // Task list.
    let mut task = Task::new("Do A".into(), "description".into(), HashMap::new());
    task.id = "1".into();
    session.state_mut().tasks_context.add_task(task);
    // Middleware context.
    session
        .state_mut()
        .middle_context
        .insert("phase".into(), serde_json::json!("persist"));
    // Reply context.
    session.state_mut().reply_context.reply_id = "reply-1".into();
    session.state_mut().reply_context.cur_iter = 3;

    store.save(&session).await.unwrap();

    // The on-disk file exists and is the only file (no temp leftovers).
    let entries: Vec<String> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["s1.json".to_string()]);
    assert!(tmp.path().join("s1.json").exists());

    // Load and verify full-state round-trip.
    let restored = store.load("s1").await.unwrap();
    assert_eq!(restored.id(), "s1");
    assert_eq!(restored.state().context_length(), 5);
    assert!(matches!(
        &restored.state().summary,
        SummaryContent::Text(s) if s == "summarized"
    ));
    assert_eq!(restored.state().tasks_context.tasks.len(), 1);
    assert_eq!(
        restored
            .state()
            .middle_context
            .get("phase")
            .and_then(|v| v.as_str()),
        Some("persist")
    );
    assert_eq!(restored.state().reply_context.reply_id, "reply-1");
    assert_eq!(restored.state().reply_context.cur_iter, 3);

    // Message content preserved.
    for msg in restored.state().context.iter() {
        assert_eq!(msg.name, "agent");
        assert_eq!(msg.role, Role::Assistant);
        assert!(!msg.content.is_empty());
    }
}

// ---------------------------------------------------------------------------
// T009 — atomic writes, corrupted files, invalid ids (quickstart 场景 6)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_atomic_write_leaves_no_temp_file() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());

    let mut session = SessionImpl::with_session_id("s2".into());
    let blocks = vec![ContentBlock::Text(TextBlock::new("hello".into()))];
    session.state_mut().append_context("agent", blocks).unwrap();

    store.save(&session).await.unwrap();

    let entries: Vec<String> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        entries.iter().all(|n| !n.ends_with(".tmp")),
        "no temp files should remain after a save: {entries:?}"
    );
    assert_eq!(entries, vec!["s2.json".to_string()]);
}

#[tokio::test]
async fn test_corrupted_file_returns_serialization_error() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("bad.json"), "{ not valid json").unwrap();

    let store = JsonFileSessionStore::new(tmp.path());
    assert!(
        matches!(
            store.load("bad").await,
            Err(SessionError::SerializationError { .. })
        ),
        "corrupted file must return SerializationError"
    );
}

#[tokio::test]
async fn test_invalid_session_id_rejected_no_path_traversal() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());

    // Save with an id containing a path separator.
    let evil = SessionImpl::with_session_id("../evil".into());
    let err = store.save(&evil).await.unwrap_err();
    assert!(matches!(err, SessionError::StorageError { .. }));
    // No file must have been written outside the store directory.
    let parent = tmp.path().parent().unwrap();
    assert!(!parent.join("evil.json").exists());

    // Load with an id containing '.' is rejected too.
    assert!(store.load("a.b").await.is_err());
    // A valid id that does not exist → NotFound (new session, not a crash).
    assert!(matches!(
        store.load("missing").await,
        Err(SessionError::NotFound { .. })
    ));
}

// ---------------------------------------------------------------------------
// T021 — list / delete semantics (quickstart 场景 8)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_ids_and_meta_sorted_by_last_active() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());

    for id in ["s-a", "s-b", "s-c"] {
        let mut session = SessionImpl::with_session_id(id.to_string());
        if id == "s-b" {
            for _ in 0..3 {
                let blocks = vec![ContentBlock::Text(TextBlock::new("msg".into()))];
                session.state_mut().append_context("agent", blocks).unwrap();
            }
        }
        store.save(&session).await.unwrap();
    }

    let ids = store.list_ids().await.unwrap();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&"s-a".to_string()));
    assert!(ids.contains(&"s-b".to_string()));
    assert!(ids.contains(&"s-c".to_string()));

    let metas = store.list_meta().await.unwrap();
    assert_eq!(metas.len(), 3);
    // Metadata is sorted by last_active descending.
    for i in 1..metas.len() {
        assert!(
            metas[i - 1].last_active >= metas[i].last_active,
            "metadata should be sorted by last_active descending"
        );
    }
    let meta_b = metas.iter().find(|m| m.session_id == "s-b").unwrap();
    assert_eq!(meta_b.message_count, 3);
}

#[tokio::test]
async fn test_delete_idempotent_and_load_not_found_after() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());

    let session = SessionImpl::with_session_id("s-del".into());
    store.save(&session).await.unwrap();

    store.delete("s-del").await.unwrap();
    // Deleting again is idempotent (no error).
    store.delete("s-del").await.unwrap();
    // Deleting a session that never existed is also Ok.
    store.delete("never-existed").await.unwrap();

    let err = store.load("s-del").await;
    assert!(
        matches!(err, Err(SessionError::NotFound { .. })),
        "load after delete must return NotFound"
    );
}
