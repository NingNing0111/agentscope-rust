use clap::Parser;
use pi_rust::config::{Cli, RuntimeConfig};
use pi_rust::session::{SessionRecord, SessionStore};

fn config(dir: &tempfile::TempDir, cwd: &tempfile::TempDir) -> RuntimeConfig {
    RuntimeConfig::from_cli(Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-session-test",
        "--workdir",
        dir.path().to_str().unwrap(),
        "--cwd",
        cwd.path().to_str().unwrap(),
    ]))
    .unwrap()
}

#[test]
fn session_record_json_round_trips_without_api_key() {
    let workdir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let config = config(&workdir, &cwd);
    let mut record = SessionRecord::new(&config);
    record.add_turn("remember hello".into(), Vec::new(), "ok".into(), None);

    let json = serde_json::to_string_pretty(&record).unwrap();
    assert!(!json.contains("sk-session-test"));
    let restored: SessionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.id, record.id);
    assert_eq!(restored.turns.len(), 1);
    assert_eq!(restored.turns[0].index, 0);
}

#[test]
fn session_store_saves_lists_loads_latest_and_selected() {
    let workdir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let config = config(&workdir, &cwd);
    let store = SessionStore::new(workdir.path().join("sessions"));
    let mut record = SessionRecord::new(&config);
    record.add_turn("hello".into(), Vec::new(), "world".into(), None);
    let id = record.id.clone();

    store.save(&record).unwrap();
    let summaries = store.list().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, id);
    assert_eq!(store.load(&id).unwrap().turns.len(), 1);
    assert_eq!(store.load_latest().unwrap().unwrap().id, id);
}

#[test]
fn session_store_reports_missing_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::new(dir.path().join("sessions"));
    assert!(store.list().unwrap().is_empty());
    assert!(store.load_latest().unwrap().is_none());
    let err = store.load("missing").unwrap_err();
    assert!(err.safe_message().contains("does not exist"));
}

#[test]
fn corrupt_session_json_is_ignored_by_listing_and_errors_on_selected_load() {
    let dir = tempfile::tempdir().unwrap();
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("bad.json"), "not json").unwrap();
    let store = SessionStore::new(sessions);
    assert!(store.list().unwrap().is_empty());
    let err = store.load("bad").unwrap_err();
    assert!(err.safe_message().contains("expected") || err.safe_message().contains("JSON"));
}

#[test]
fn session_summary_uses_assistant_reply() {
    let workdir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let config = config(&workdir, &cwd);
    let mut record = SessionRecord::new(&config);
    // A long user input must not become the session summary; the reply does.
    record.add_turn("x".repeat(200), Vec::new(), "short reply".into(), None);
    assert_eq!(record.summary.as_deref(), Some("short reply"));
    // Multi-line replies collapse into a single line.
    record.add_turn("q".into(), Vec::new(), "line one\nline two".into(), None);
    assert_eq!(record.summary.as_deref(), Some("line one line two"));
}

#[test]
fn session_tasks_snapshot_round_trips() {
    let workdir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let config = config(&workdir, &cwd);
    let mut record = SessionRecord::new(&config);
    let tasks = serde_json::json!({
        "tasks": [
            { "id": "1", "subject": "fix bug", "state": "in_progress" }
        ]
    });
    record.snapshot_tasks(tasks.clone());

    let json = serde_json::to_string(&record).unwrap();
    assert!(json.contains("tasks_snapshot"), "{json}");
    let restored: SessionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.tasks_snapshot, Some(tasks));
    // A fresh record has no snapshot field serialized.
    let fresh = SessionRecord::new(&config);
    let fresh_json = serde_json::to_string(&fresh).unwrap();
    assert!(!fresh_json.contains("tasks_snapshot"), "{fresh_json}");
}
