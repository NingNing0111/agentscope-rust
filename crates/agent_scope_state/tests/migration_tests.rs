//! Integration tests for legacy format migration.
//! T109

use agent_scope_state::AgentState;

#[test]
fn test_legacy_migration_moves_reply_id_to_reply_context() {
    let legacy = r#"{
        "session_id": "abc123",
        "reply_id": "old-reply-id",
        "cur_iter": 5,
        "summary": "",
        "context": [],
        "reply_context": {},
        "permission_context": {},
        "tool_context": {
            "max_cache_files": 100,
            "max_cache_bytes": 25000.0,
            "read_file_cache": [],
            "activated_groups": []
        },
        "tasks_context": {"tasks": []},
        "middle_context": {}
    }"#;

    let state = AgentState::from_legacy_json(legacy).unwrap();
    assert_eq!(state.session_id, "abc123");
    assert_eq!(state.reply_context.reply_id, "old-reply-id");
    assert_eq!(state.reply_context.cur_iter, 5);
}

#[test]
fn test_legacy_migration_only_reply_id() {
    let legacy = r#"{
        "session_id": "session-only-reply",
        "reply_id": "only-reply-id",
        "summary": "",
        "context": [],
        "reply_context": {},
        "permission_context": {},
        "tool_context": {
            "max_cache_files": 100,
            "max_cache_bytes": 25000.0,
            "read_file_cache": [],
            "activated_groups": []
        },
        "tasks_context": {"tasks": []},
        "middle_context": {}
    }"#;

    let state = AgentState::from_legacy_json(legacy).unwrap();
    assert_eq!(state.session_id, "session-only-reply");
    assert_eq!(state.reply_context.reply_id, "only-reply-id");
    assert_eq!(state.reply_context.cur_iter, 0); // default
}

#[test]
fn test_legacy_migration_only_cur_iter() {
    let legacy = r#"{
        "session_id": "session-only-iter",
        "cur_iter": 42,
        "summary": "",
        "context": [],
        "reply_context": {},
        "permission_context": {},
        "tool_context": {
            "max_cache_files": 100,
            "max_cache_bytes": 25000.0,
            "read_file_cache": [],
            "activated_groups": []
        },
        "tasks_context": {"tasks": []},
        "middle_context": {}
    }"#;

    let state = AgentState::from_legacy_json(legacy).unwrap();
    assert_eq!(state.session_id, "session-only-iter");
    assert_eq!(state.reply_context.cur_iter, 42);
    // reply_id should be auto-generated
    assert!(!state.reply_context.reply_id.is_empty());
}

#[test]
fn test_legacy_migration_preserves_existing_reply_context_fields() {
    let legacy = r#"{
        "session_id": "merged",
        "reply_id": "top-level-reply",
        "cur_iter": 3,
        "summary": "",
        "context": [],
        "reply_context": {
            "reply_id": "nested-reply",
            "cur_iter": 0,
            "structured_schema": {"type": "object"}
        },
        "permission_context": {},
        "tool_context": {
            "max_cache_files": 100,
            "max_cache_bytes": 25000.0,
            "read_file_cache": [],
            "activated_groups": []
        },
        "tasks_context": {"tasks": []},
        "middle_context": {}
    }"#;

    let state = AgentState::from_legacy_json(legacy).unwrap();
    // Nested reply_context values take precedence if already present
    assert_eq!(state.reply_context.reply_id, "nested-reply");
    assert_eq!(state.reply_context.cur_iter, 0);
    // structured_schema is preserved
    assert!(state.reply_context.structured_schema.is_some());
}

#[test]
fn test_legacy_migration_without_legacy_fields_is_idempotent() {
    let modern = r#"{
        "session_id": "modern",
        "summary": "test",
        "context": [],
        "reply_context": {
            "reply_id": "modern-reply",
            "cur_iter": 10
        },
        "permission_context": {},
        "tool_context": {
            "max_cache_files": 100,
            "max_cache_bytes": 25000.0,
            "read_file_cache": [],
            "activated_groups": []
        },
        "tasks_context": {"tasks": []},
        "middle_context": {}
    }"#;

    let state = AgentState::from_legacy_json(modern).unwrap();
    assert_eq!(state.session_id, "modern");
    assert_eq!(state.reply_context.reply_id, "modern-reply");
    assert_eq!(state.reply_context.cur_iter, 10);
}

#[test]
fn test_legacy_migration_with_context_messages() {
    use agent_scope_message::block::TextBlock;
    use agent_scope_message::msg::{Msg, Role};

    // Create a state with context messages first
    let mut state = AgentState::new();
    let msg = Msg::new(
        "agent".into(),
        vec![agent_scope_message::block::ContentBlock::Text(
            TextBlock::new("hello".into()),
        )],
        Role::Assistant,
    )
    .unwrap();
    state.context.push(msg);

    // Serialize and add legacy fields to the JSON
    let mut value = serde_json::to_value(&state).unwrap();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("reply_id".into(), serde_json::json!("legacy-reply"));
        obj.insert("cur_iter".into(), serde_json::json!(99));
    }

    let legacy_json = serde_json::to_string(&value).unwrap();
    let restored = AgentState::from_legacy_json(&legacy_json).unwrap();

    // Migration preserves existing reply_context values. Since reply_context
    // already has reply_id and cur_iter (from serialized AgentState::new()),
    // legacy top-level fields don't overwrite them.
    // Context messages are preserved through the migration
    assert_eq!(restored.context_length(), 1);
    assert_eq!(restored.context[0].get_text_content(" ").unwrap(), "hello");
}

#[test]
fn test_legacy_migration_invalid_json_returns_error() {
    let result = AgentState::from_legacy_json("not valid json");
    assert!(result.is_err());
}
