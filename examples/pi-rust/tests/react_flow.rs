use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::factory::user_msg;
use agent_scope_message::{ContentBlock, Msg, TextBlock, ToolCallBlock};
use agent_scope_model::{ChatModel, ChatResponse, ModelCallResult, ModelError, ToolChoice};
use agent_scope_tool::ToolKit;
use agent_scope_workspace::Skill;
use clap::Parser;
use futures::{Stream, stream};
use pi_rust::config::{Cli, RuntimeConfig};
use pi_rust::session::{SessionRecord, SessionStore};
use pi_rust::tools::{ToolState, build_toolkit, resolve_workspace_path};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
enum ScriptedResponse {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        input: String,
    },
}

struct ScriptedModel {
    responses: Mutex<Vec<ScriptedResponse>>,
    stream: bool,
}

#[async_trait::async_trait]
impl ChatModel for ScriptedModel {
    fn model_name(&self) -> &str {
        "scripted"
    }
    fn stream_enabled(&self) -> bool {
        self.stream
    }

    async fn call_api(
        &self,
        _: &str,
        _: &[Msg],
        _: Option<&[JsonValue]>,
        _: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        let next = self.responses.lock().unwrap().remove(0);
        let mut response = ChatResponse::default();
        match next {
            ScriptedResponse::Text(text) => response
                .content
                .push(ContentBlock::Text(TextBlock::new(text))),
            ScriptedResponse::ToolCall { id, name, input } => {
                response
                    .content
                    .push(ContentBlock::ToolCall(ToolCallBlock::new(id, name, input)));
            }
        }
        if self.stream {
            let s: Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>> =
                Box::pin(stream::iter(vec![Ok(response)]));
            Ok(ModelCallResult::Stream(s))
        } else {
            Ok(ModelCallResult::Complete(response))
        }
    }
}

fn demo_skill() -> Skill {
    Skill {
        name: "demo".into(),
        description: "Demo skill".into(),
        dir: "demo".into(),
        markdown: "# Demo\nFollow demo skill instructions.".into(),
        updated_at: 0.0,
    }
}

fn agent_with(script: Vec<ScriptedResponse>, toolkit: ToolKit) -> ReActAgent {
    let model = Arc::new(ScriptedModel {
        responses: Mutex::new(script),
        stream: false,
    });
    let config = AgentConfig::builder()
        .name("test")
        .model(model)
        .toolkit(toolkit)
        .build()
        .unwrap();
    ReActAgent::new(
        config,
        ReActConfig {
            max_iters: 5,
            ..Default::default()
        },
        ContextConfig::default(),
        vec![],
    )
    .unwrap()
}

#[tokio::test]
async fn react_flow_reads_file_then_answers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    let toolkit = build_toolkit(
        ToolState::new(dir.path().canonicalize().unwrap(), 2),
        vec![],
        dir.path().join("skills"),
    );
    let agent = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Read".into(),
                input: r#"{"path":"main.rs"}"#.into(),
            },
            ScriptedResponse::Text("main contains a main function".into()),
        ],
        toolkit,
    );
    let reply = agent
        .reply(Some(vec![user_msg("user", "read main").unwrap()]))
        .await
        .unwrap();
    assert_eq!(
        reply.get_text_content("").unwrap(),
        "main contains a main function"
    );
}

#[tokio::test]
async fn react_flow_writes_and_edits_file() {
    let dir = tempfile::tempdir().unwrap();
    let toolkit = build_toolkit(
        ToolState::new(dir.path().canonicalize().unwrap(), 2),
        vec![],
        dir.path().join("skills"),
    );
    let agent = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Write".into(),
                input: r#"{"path":"hello.txt","content":"Hello, World!"}"#.into(),
            },
            ScriptedResponse::ToolCall {
                id: "tc2".into(),
                name: "Edit".into(),
                input: r#"{"path":"hello.txt","old_string":"World","new_string":"Rust"}"#.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
        toolkit,
    );
    let reply = agent
        .reply(Some(vec![user_msg("user", "create and edit").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.get_text_content("").unwrap(), "done");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "Hello, Rust!"
    );
}

#[tokio::test]
async fn react_flow_executes_safe_bash() {
    let dir = tempfile::tempdir().unwrap();
    let toolkit = build_toolkit(
        ToolState::new(dir.path().canonicalize().unwrap(), 2),
        vec![],
        dir.path().join("skills"),
    );
    let agent = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Bash".into(),
                input: r#"{"command":"pwd"}"#.into(),
            },
            ScriptedResponse::Text("pwd returned".into()),
        ],
        toolkit,
    );
    let reply = agent
        .reply(Some(vec![user_msg("user", "pwd").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.get_text_content("").unwrap(), "pwd returned");
}

#[tokio::test]
async fn react_flow_uses_skill_tool_when_loaded() {
    let dir = tempfile::tempdir().unwrap();
    // SkillViewer 实时扫描 workspace/skills:skill 必须真实存在于磁盘上。
    let skills_dir = dir.path().join("skills");
    std::fs::create_dir_all(skills_dir.join("demo")).unwrap();
    std::fs::write(
        skills_dir.join("demo").join("SKILL.md"),
        "---\nname: demo\ndescription: Demo skill\n---\n\n# Demo\nFollow demo skill instructions.",
    )
    .unwrap();
    let toolkit = build_toolkit(
        ToolState::new(dir.path().canonicalize().unwrap(), 2),
        vec![demo_skill()],
        skills_dir,
    );
    let agent = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Skill".into(),
                input: r#"{"skill":"demo"}"#.into(),
            },
            ScriptedResponse::Text("followed skill".into()),
        ],
        toolkit,
    );
    let reply = agent
        .reply(Some(vec![user_msg("user", "use demo skill").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.get_text_content("").unwrap(), "followed skill");
}

#[tokio::test]
async fn coding_flow_edits_and_verifies_once() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, World!").unwrap();
    let toolkit = build_toolkit(
        ToolState::new(dir.path().canonicalize().unwrap(), 2),
        vec![],
        dir.path().join("skills"),
    );
    let agent = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Read".into(),
                input: r#"{"path":"hello.txt"}"#.into(),
            },
            ScriptedResponse::ToolCall {
                id: "tc2".into(),
                name: "Edit".into(),
                input: r#"{"path":"hello.txt","old_string":"World","new_string":"Rust"}"#.into(),
            },
            ScriptedResponse::ToolCall {
                id: "tc3".into(),
                name: "Bash".into(),
                input: r#"{"command":"grep -q Rust hello.txt"}"#.into(),
            },
            ScriptedResponse::Text(
                "Summary: updated greeting\nFiles changed: hello.txt\nVerification: grep passed\nRemaining risks: none".into(),
            ),
        ],
        toolkit,
    );
    let reply = agent
        .reply(Some(vec![user_msg("user", "change greeting").unwrap()]))
        .await
        .unwrap();
    assert!(reply.get_text_content("").unwrap().contains("Verification"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "Hello, Rust!"
    );
}

#[tokio::test]
async fn coding_flow_iterates_after_failed_verification() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, World!").unwrap();
    let toolkit = build_toolkit(
        ToolState::new(dir.path().canonicalize().unwrap(), 2),
        vec![],
        dir.path().join("skills"),
    );
    let agent = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Edit".into(),
                input: r#"{"path":"hello.txt","old_string":"World","new_string":"Ferris"}"#.into(),
            },
            ScriptedResponse::ToolCall {
                id: "tc2".into(),
                name: "Bash".into(),
                input: r#"{"command":"grep -q Rust hello.txt"}"#.into(),
            },
            ScriptedResponse::ToolCall {
                id: "tc3".into(),
                name: "Edit".into(),
                input: r#"{"path":"hello.txt","old_string":"Ferris","new_string":"Rust"}"#.into(),
            },
            ScriptedResponse::ToolCall {
                id: "tc4".into(),
                name: "Bash".into(),
                input: r#"{"command":"grep -q Rust hello.txt"}"#.into(),
            },
            ScriptedResponse::Text(
                "Summary: fixed after verification failure\nFiles changed: hello.txt\nVerification: grep passed\nRemaining risks: none".into(),
            ),
        ],
        toolkit,
    );
    let reply = agent
        .reply(Some(vec![
            user_msg("user", "iterate until verified").unwrap(),
        ]))
        .await
        .unwrap();
    assert!(reply.get_text_content("").unwrap().contains("fixed after"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "Hello, Rust!"
    );
}

#[test]
fn session_resume_reconstructs_prior_turn_context() {
    let workdir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let config = RuntimeConfig::from_cli(Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-react-flow",
        "--workdir",
        workdir.path().to_str().unwrap(),
        "--cwd",
        cwd.path().to_str().unwrap(),
    ]))
    .unwrap();
    let store = SessionStore::new(workdir.path().join("sessions"));
    let mut record = SessionRecord::new(&config);
    record.add_turn(
        "remember that the greeting is Hello Rust".into(),
        Vec::new(),
        "I will remember Hello Rust.".into(),
        None,
    );
    let id = record.id.clone();
    store.save(&record).unwrap();

    let resumed = store.load(&id).unwrap();
    assert_eq!(resumed.turns.len(), 1);
    assert_eq!(
        resumed.turns[0].user_input,
        "remember that the greeting is Hello Rust"
    );
    assert_eq!(
        resumed.turns[0].assistant_text,
        "I will remember Hello Rust."
    );
}

#[test]
fn long_input_and_context_growth_are_serializable() {
    let workdir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let config = RuntimeConfig::from_cli(Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-long-input",
        "--workdir",
        workdir.path().to_str().unwrap(),
        "--cwd",
        cwd.path().to_str().unwrap(),
    ]))
    .unwrap();
    let mut record = SessionRecord::new(&config);
    record.add_turn("x".repeat(32_000), Vec::new(), "ok".into(), None);
    for idx in 0..25 {
        record.add_turn(
            format!("message {idx}"),
            Vec::new(),
            format!("reply {idx}"),
            None,
        );
    }
    let json = serde_json::to_string(&record).unwrap();
    assert!(json.contains("message 24"));
    let restored: SessionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.turns.len(), 26);
    assert_eq!(restored.turns[25].index, 25);
}

#[test]
fn memory_and_rag_disable_flags_build_runtime_config() {
    let workdir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let disabled = RuntimeConfig::from_cli(Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-flags",
        "--workdir",
        workdir.path().to_str().unwrap(),
        "--cwd",
        cwd.path().to_str().unwrap(),
        "--no-memory",
        "--no-rag",
    ]))
    .unwrap();
    assert!(disabled.no_memory);
    assert!(disabled.no_rag);

    let enabled = RuntimeConfig::from_cli(Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-flags",
        "--workdir",
        workdir.path().to_str().unwrap(),
        "--cwd",
        cwd.path().to_str().unwrap(),
    ]))
    .unwrap();
    assert!(!enabled.no_memory);
    assert!(!enabled.no_rag);
}

#[tokio::test]
async fn approval_gate_denies_then_allows_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "old").unwrap();
    let path = dir.path().join("hello.txt");
    let approvals = Arc::new(Mutex::new(HashSet::new()));
    let make_state = || {
        let mut state = ToolState::new(dir.path().canonicalize().unwrap(), 2);
        state.approvals = Arc::clone(&approvals);
        state
    };
    let overwrite_input =
        r#"{"path":"hello.txt","content":"new","overwrite":true,"confirmed":false}"#;

    // First turn: the overwrite is denied (no approval yet); the file is intact.
    let agent1 = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Write".into(),
                input: overwrite_input.into(),
            },
            ScriptedResponse::Text("needs approval".into()),
        ],
        build_toolkit(make_state(), vec![], dir.path().join("skills")),
    );
    let first = agent1
        .reply(Some(vec![user_msg("user", "overwrite hello").unwrap()]))
        .await
        .unwrap();
    assert_eq!(first.get_text_content("").unwrap(), "needs approval");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");

    // Simulate the REPL approval: record the fingerprint, then the same call
    // succeeds on the retry. Use resolve_workspace_path so the fingerprint
    // matches the canonical path the tool derives.
    let fp_path = resolve_workspace_path(&make_state().cwd, "hello.txt").unwrap();
    approvals
        .lock()
        .unwrap()
        .insert(format!("write:{}", fp_path.display()));
    let agent2 = agent_with(
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "Write".into(),
                input: overwrite_input.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
        build_toolkit(make_state(), vec![], dir.path().join("skills")),
    );
    let second = agent2
        .reply(Some(vec![user_msg("user", "overwrite hello").unwrap()]))
        .await
        .unwrap();
    assert_eq!(second.get_text_content("").unwrap(), "done");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}
