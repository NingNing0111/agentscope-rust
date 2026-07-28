//! Golden snapshot diff test framework.
//! T112
//!
//! Reads Python-generated fixture JSON, compares Rust serialization output
//! (with timestamp/UUID normalization), and reports mismatches.

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// The top-level format of each fixture JSON file.
#[derive(Debug, Deserialize)]
struct FixtureFile {
    #[serde(rename = "_fixture_name")]
    #[allow(dead_code)]
    name: String,
    data: serde_json::Value,
}

/// Recursively remove `id`, `created_at`, `finished_at`, `updated_at` fields.
/// These are auto-generated and differ between Python and Rust runs.
fn remove_dynamic_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("id");
            map.remove("created_at");
            map.remove("finished_at");
            map.remove("updated_at");
            map.remove("reply_id");
            for val in map.values_mut() {
                remove_dynamic_fields(val);
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr.iter_mut() {
                remove_dynamic_fields(val);
            }
        }
        _ => {}
    }
}

fn normalize(value: &serde_json::Value) -> serde_json::Value {
    let mut copy = value.clone();
    remove_dynamic_fields(&mut copy);
    copy
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")) // crates/agent_scope_state
        .parent() // crates
        .unwrap()
        .parent() // workspace root
        .unwrap()
        .join("tests")
        .join("compatibility")
        .join("fixtures")
}

fn load_fixture(name: &str) -> FixtureFile {
    let path = fixtures_dir().join(name);
    let content = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "Fixture not found: {}. Run generate_fixtures.py first.",
            path.display()
        )
    });
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", path.display(), e))
}

fn assert_matches_fixture<T: serde::Serialize>(fixture_name: &str, rust_value: &T) {
    let fixture = load_fixture(fixture_name);
    let rust_json = serde_json::to_value(rust_value).unwrap();
    let normalized_rust = normalize(&rust_json);
    let normalized_fixture = normalize(&fixture.data);

    assert_eq!(
        normalized_rust,
        normalized_fixture,
        "Snapshot mismatch for fixture '{}'.\n\
         Rust (normalized):  {}\n\
         Fixture (normalized): {}",
        fixture_name,
        serde_json::to_string_pretty(&normalized_rust).unwrap(),
        serde_json::to_string_pretty(&normalized_fixture).unwrap(),
    );
}

// ── Tests ────────────────────────────────────────────────────────

mod snapshot_tests {
    use super::*;

    // Msg snapshots
    #[test]
    fn test_msg_user_text_matches_snapshot() {
        let msg =
            agent_scope_message::factory::user_msg("user1", "Hello, what is the weather?").unwrap();
        let value = serde_json::json!({
            "name": msg.name,
            "role": serde_json::to_value(msg.role).unwrap(),
            "content": msg.content,
            "metadata": msg.metadata,
        });
        assert_matches_fixture("msg_user_text.json", &value);
    }

    #[test]
    fn test_msg_assistant_text_matches_snapshot() {
        let msg =
            agent_scope_message::factory::assistant_msg("assistant", "The weather is sunny today.");
        let value = serde_json::json!({
            "name": msg.name,
            "role": serde_json::to_value(msg.role).unwrap(),
            "content": msg.content,
            "metadata": msg.metadata,
        });
        assert_matches_fixture("msg_assistant_text.json", &value);
    }

    #[test]
    fn test_msg_system_text_matches_snapshot() {
        let msg =
            agent_scope_message::factory::system_msg("system", "You are a helpful assistant.")
                .unwrap();
        let value = serde_json::json!({
            "name": msg.name,
            "role": serde_json::to_value(msg.role).unwrap(),
            "content": msg.content,
            "metadata": msg.metadata,
        });
        assert_matches_fixture("msg_system_text.json", &value);
    }

    // ContentBlock snapshots
    #[test]
    fn test_content_block_text_matches_snapshot() {
        let value = serde_json::json!({
            "type": "text",
            "text": "Hello, world!",
        });
        assert_matches_fixture("content_block_text.json", &value);
    }

    #[test]
    fn test_content_block_thinking_matches_snapshot() {
        let value = serde_json::json!({
            "type": "thinking",
            "thinking": "Let me reason about this...",
        });
        assert_matches_fixture("content_block_thinking.json", &value);
    }

    #[test]
    fn test_content_block_hint_matches_snapshot() {
        let value = serde_json::json!({
            "type": "hint",
            "hint": "Please respond in JSON format",
            "source": "system",
        });
        assert_matches_fixture("content_block_hint.json", &value);
    }

    #[test]
    fn test_content_block_tool_call_matches_snapshot() {
        let value = serde_json::json!({
            "type": "tool_call",
            "id": "call-abc",
            "name": "get_weather",
            "input": r#"{"city": "Beijing"}"#,
            "state": "pending",
        });
        assert_matches_fixture("content_block_tool_call.json", &value);
    }

    // AgentState snapshot
    #[test]
    fn test_agent_state_default_matches_snapshot() {
        use agent_scope_state::AgentState;
        let state = AgentState::with_session_id("test-session-001".into());
        let value = serde_json::json!({
            "session_id": state.session_id,
            "summary": "",
            "context": [],
            "reply_context": {
                "reply_id": state.reply_context.reply_id,
                "cur_iter": 0,
            },
            "permission_context": {},
            "tool_context": {
                "max_cache_files": 100,
                "max_cache_bytes": 25000.0,
                "read_file_cache": [],
                "activated_groups": [],
            },
            "tasks_context": {"tasks": []},
            "middle_context": {},
        });
        let fixture = load_fixture("agent_state_default.json");
        let rust_json = serde_json::to_value(&value).unwrap();
        let normalized_rust = normalize(&rust_json);
        let normalized_fixture = normalize(&fixture.data);
        assert_eq!(
            normalized_rust, normalized_fixture,
            "AgentState snapshot mismatch"
        );
    }

    // Task snapshot
    #[test]
    fn test_task_pending_matches_snapshot() {
        use agent_scope_state::task::Task;
        let task = Task::new(
            "Implement login".into(),
            "Add OAuth2 authentication flow".into(),
            std::collections::HashMap::new(),
        );
        let value = serde_json::json!({
            "subject": task.subject,
            "description": task.description,
            "metadata": {},
            "state": "pending",
            "blocks": [],
            "blocked_by": [],
        });
        assert_matches_fixture("task_pending.json", &value);
    }
}
