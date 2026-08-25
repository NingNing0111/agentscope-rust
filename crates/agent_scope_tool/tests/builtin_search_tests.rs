//! US3 — Glob/Grep/Skill search & discovery (FR-013~016, FR-020~021, SC-007).
//!
//! Mirrors quickstart scenario 3 and contracts/glob.md, grep.md, skill.md.

mod common;

use std::sync::Arc;

use agent_scope_message::ToolResultState;
use agent_scope_tool::Tool;
use agent_scope_tool::builtin::{FindTool, GlobTool, GrepTool, SkillTool};

use common::{ctx_in, text_of, write_ws_file};

fn is_success(block: &agent_scope_message::ToolResultBlock) -> bool {
    block.state == ToolResultState::Success
}

fn state_of(block: &agent_scope_message::ToolResultBlock) -> ToolResultState {
    block.state.clone()
}

// ── Glob ──

#[tokio::test]
async fn glob_results_confined_to_workspace() {
    let h = ctx_in(&[]);
    // Create files both inside and outside the workspace.
    std::fs::create_dir_all(std::path::Path::new(&h.workdir).join("src")).unwrap();
    std::fs::write(
        std::path::Path::new(&h.workdir).join("src/a.rs"),
        "fn a() {}\n",
    )
    .unwrap();
    std::fs::write(std::path::Path::new(&h.workdir).join("b.rs"), "fn b() {}\n").unwrap();

    let tool = GlobTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "**/*.rs" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    let text = text_of(&block);
    assert!(text.contains("a.rs"), "got: {text}");
    assert!(text.contains("b.rs"), "got: {text}");
    // Every returned path stays inside the workspace root.
    for line in text.lines() {
        if line.contains('.') && line.contains("rs") {
            assert!(
                line.starts_with("src/") || line == "b.rs",
                "path escaped workspace: {line}"
            );
        }
    }
}

#[tokio::test]
async fn glob_no_match_is_success() {
    let h = ctx_in(&[]);
    let tool = GlobTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "**/*.zzz" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    assert!(
        text_of(&block).contains("No files found matching pattern"),
        "got: {}",
        text_of(&block)
    );
}

#[tokio::test]
async fn glob_path_escape_rejected() {
    let h = ctx_in(&[]);
    let tool = GlobTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "**/*.rs", "path": "/etc" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("path_outside_workspace"));
}

// ── Grep ──

#[tokio::test]
async fn grep_content_mode_with_line_numbers() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "main.rs", "fn main() {\n    println!(\"Error!\");\n}\n");
    let tool = GrepTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({
            "pattern": "Error",
            "output_mode": "content"
        }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    assert!(
        text_of(&block).contains("println"),
        "got: {}",
        text_of(&block)
    );
}

#[tokio::test]
async fn grep_count_mode() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "Error\nno\nError\n");
    let tool = GrepTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "Error", "output_mode": "count" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    assert!(text_of(&block).contains("2"), "got: {}", text_of(&block));
}

#[tokio::test]
async fn grep_head_limit_bounds_results() {
    let h = ctx_in(&[]);
    // 20 matching lines in one file.
    let content = (0..20)
        .map(|i| format!("hit {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    write_ws_file(&h, "a.txt", &content);
    let tool = GrepTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "hit", "output_mode": "content", "head_limit": 5 }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    // At most ~5 matching lines + optional note lines.
    let text = text_of(&block);
    let hit_count = text.lines().filter(|l| l.contains("hit")).count();
    assert!(
        hit_count <= 5,
        "head_limit not honored: got {hit_count} hits"
    );
}

#[tokio::test]
async fn grep_no_match_is_success() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "nothing here\n");
    let tool = GrepTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "zzz" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    assert!(
        text_of(&block).contains("No matches found"),
        "got: {}",
        text_of(&block)
    );
}

#[tokio::test]
async fn grep_invalid_pattern_rejected() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "x\n");
    let tool = GrepTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "([unclosed" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("invalid_pattern"));
}

#[tokio::test]
async fn grep_pi_mode_supports_literal_ignore_case_and_limit() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "Alpha+Beta\nalpha+beta\nAlphaZBeta\n");
    let tool = GrepTool::new_pi(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({
            "pattern": "Alpha+Beta",
            "literal": true,
            "ignoreCase": true,
            "limit": 1
        }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(block.name, "grep");
    assert_eq!(state_of(&block), ToolResultState::Success);
    let text = text_of(&block);
    assert!(
        text.contains("Alpha+Beta") || text.contains("alpha+beta"),
        "got: {text}"
    );
    assert!(
        !text.contains("AlphaZBeta"),
        "literal search matched regex-style text: {text}"
    );
    let match_lines = text
        .lines()
        .filter(|line| line.contains("+Beta") || line.contains("+beta"))
        .count();
    assert_eq!(
        match_lines, 1,
        "limit should cap pi grep content results: {text}"
    );
}

#[tokio::test]
async fn grep_pi_mode_defaults_to_content_output() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "needle here\n");
    let tool = GrepTool::new_pi(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "pattern": "needle" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(state_of(&block), ToolResultState::Success);
    let text = text_of(&block);
    assert!(text.contains("needle here"), "got: {text}");
}

#[tokio::test]
async fn grep_pi_mode_rejects_non_object_input() {
    let h = ctx_in(&[]);
    let tool = GrepTool::new_pi(h.ctx.clone());

    let out = tool.call(serde_json::json!("needle")).await.unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("missing required 'pattern'"));
}

#[tokio::test]
async fn grep_pi_literal_preserves_trailing_space() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "foo \\nfoo \nfoo\n");
    let tool = GrepTool::new_pi(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "pattern": "foo ", "literal": true }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(state_of(&block), ToolResultState::Success);
    let text = text_of(&block);
    assert!(text.contains("foo "), "got: {text}");
    assert!(!text.contains("invalid_pattern"), "got: {text}");
}

// ── Find ──

#[tokio::test]
async fn find_matches_files_with_glob_and_path() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "src/a.rs", "fn a() {}\n");
    write_ws_file(&h, "src/b.txt", "b\n");
    write_ws_file(&h, "tests/c.rs", "fn c() {}\n");

    let tool = FindTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "*.rs", "path": "src" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(state_of(&block), ToolResultState::Success);
    let text = text_of(&block);
    assert!(text.lines().any(|line| line == "src/a.rs"), "got: {text}");
    assert!(!text.contains("b.txt"), "got: {text}");
    assert!(
        !text.contains("c.rs"),
        "path filter leaked outside src: {text}"
    );
}

#[tokio::test]
async fn find_skips_git_node_modules_and_directories() {
    let h = ctx_in(&[]);
    write_ws_file(&h, ".git/config", "ignored\n");
    write_ws_file(&h, "node_modules/pkg/index.js", "ignored\n");
    write_ws_file(&h, "src/index.js", "ok\n");
    std::fs::create_dir_all(std::path::Path::new(&h.workdir).join("src/dir.js")).unwrap();

    let tool = FindTool::new(h.ctx.clone());
    let out = tool
        .call(serde_json::json!({ "pattern": "**/*.js" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(state_of(&block), ToolResultState::Success);
    let text = text_of(&block);
    assert!(text.contains("src/index.js"), "got: {text}");
    assert!(
        !text.contains("node_modules"),
        "node_modules must be skipped: {text}"
    );
    assert!(!text.contains(".git"), ".git must be skipped: {text}");
    assert!(
        !text.contains("dir.js"),
        "directories must not be returned: {text}"
    );
}

#[tokio::test]
async fn find_limit_and_invalid_inputs_are_bounded() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "a\n");
    write_ws_file(&h, "b.txt", "b\n");
    let tool = FindTool::new(h.ctx.clone());

    let limited = match tool
        .call(serde_json::json!({ "pattern": "*.txt", "limit": 1 }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&limited), ToolResultState::Success);
    let text = text_of(&limited);
    assert_eq!(
        text.lines().filter(|line| line.ends_with(".txt")).count(),
        1
    );
    assert!(text.contains("truncated"), "got: {text}");

    let escaped = match tool
        .call(serde_json::json!({ "pattern": "*.txt", "path": "../outside" }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&escaped), ToolResultState::Error);
    assert!(text_of(&escaped).contains("path_outside_workspace"));

    let invalid = match tool
        .call(serde_json::json!({ "pattern": "[" }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&invalid), ToolResultState::Error);
    assert!(text_of(&invalid).contains("invalid_pattern"));
}

// ── Skill ──

#[tokio::test]
async fn skill_exact_name_hit() {
    let h = ctx_in(&[]);
    let skills = Arc::new({
        let mut map = std::collections::HashMap::new();
        map.insert(
            "example-skill".to_string(),
            agent_scope_workspace::Skill {
                name: "example-skill".into(),
                description: "An example".into(),
                dir: "/tmp/x".into(),
                markdown: "# Example".into(),
                updated_at: 0.0,
            },
        );
        map
    });
    let tool = SkillTool::new(h.ctx.clone(), Box::new(move |_| skills.as_ref().clone()));
    let out = tool
        .call(serde_json::json!({ "skill": "example-skill" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert!(is_success(&block));
    assert!(text_of(&block).contains("# Example"));
}

#[tokio::test]
async fn skill_not_found_error() {
    let h = ctx_in(&[]);
    let tool = SkillTool::new(
        h.ctx.clone(),
        Box::new(|_| std::collections::HashMap::new()),
    );
    let out = tool
        .call(serde_json::json!({ "skill": "no-such" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(
        text_of(&block).contains("SkillNotFoundError"),
        "got: {}",
        text_of(&block)
    );
}
