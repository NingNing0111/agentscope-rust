use agent_scope_message::{ToolCallBlock, ToolOutput};
use agent_scope_tool::ToolExecOutput;
use agent_scope_workspace::Skill;
use pi_rust::tools::{
    BashInput, EditInput, PermissionLevel, ReadInput, ToolState, WriteInput, bash_tool,
    build_toolkit, classify_bash_permission, classify_write_permission, edit_tool,
    is_destructive_command, read_tool, resolve_workspace_path, truncate_output, write_tool,
};

fn state(dir: &tempfile::TempDir) -> ToolState {
    ToolState::new(dir.path().canonicalize().unwrap(), 1)
}

fn demo_skill() -> Skill {
    Skill {
        name: "demo".into(),
        description: "Demo skill".into(),
        dir: "demo".into(),
        markdown: "# Demo\nUse this skill.".into(),
        updated_at: 0.0,
    }
}

#[tokio::test]
async fn toolkit_exposes_skill_only_when_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let without_skills = build_toolkit(state(&dir), vec![]);
    assert!(!without_skills.contains("Skill"));
    assert!(without_skills.get_skill_instructions(None).is_empty());

    let with_skills = build_toolkit(state(&dir), vec![demo_skill()]);
    assert!(with_skills.contains("Skill"));
    let instructions = with_skills.get_skill_instructions(None);
    assert!(instructions.contains("<agent-skills>"));
    assert!(instructions.contains("demo"));
    assert!(instructions.contains("Demo skill"));

    let output = with_skills
        .call_tool(&ToolCallBlock::new(
            "tc-skill".into(),
            "Skill".into(),
            r#"{"skill":"demo"}"#.into(),
        ))
        .await
        .unwrap();
    let ToolExecOutput::Complete(block) = output else {
        panic!("Skill should return a complete result");
    };
    let ToolOutput::Text(text) = block.output else {
        panic!("Skill should return text output");
    };
    assert!(text.contains("# Demo"));
}

#[test]
fn path_validation_rejects_workspace_escape() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(&dir);
    let err = resolve_workspace_path(&state.cwd, "../outside.txt").unwrap_err();
    assert_eq!(err.error.unwrap().code, "path_outside_workspace");
}

#[test]
fn read_tool_returns_line_numbered_utf8_content() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "a\nb\nc\n").unwrap();
    let result = read_tool(
        &state(&dir),
        ReadInput {
            path: "hello.txt".into(),
            offset: Some(1),
            limit: Some(1),
        },
    );
    assert!(result.ok);
    assert!(result.content.unwrap().contains("2\tb"));
}

#[test]
fn read_tool_rejects_missing_and_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    let missing = read_tool(
        &state(&dir),
        ReadInput {
            path: "missing.txt".into(),
            offset: None,
            limit: None,
        },
    );
    assert_eq!(missing.error.unwrap().code, "file_not_found");
    std::fs::write(dir.path().join("bin.dat"), [0xff, 0xfe]).unwrap();
    let binary = read_tool(
        &state(&dir),
        ReadInput {
            path: "bin.dat".into(),
            offset: None,
            limit: None,
        },
    );
    assert_eq!(binary.error.unwrap().code, "unsupported_file_type");
}

#[test]
fn write_tool_creates_and_rejects_existing_without_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(&dir);
    let first = write_tool(
        &state,
        WriteInput {
            path: "nested/hello.txt".into(),
            content: "Hello".into(),
            overwrite: false,
            confirmed: false,
        },
    );
    assert!(first.ok);
    let second = write_tool(
        &state,
        WriteInput {
            path: "nested/hello.txt".into(),
            content: "World".into(),
            overwrite: false,
            confirmed: false,
        },
    );
    assert_eq!(second.error.unwrap().code, "file_exists");
}

#[test]
fn write_overwrite_requires_confirmation_then_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "old").unwrap();
    let state = state(&dir);
    let path = resolve_workspace_path(&state.cwd, "hello.txt").unwrap();
    let input = WriteInput {
        path: "hello.txt".into(),
        content: "new".into(),
        overwrite: true,
        confirmed: false,
    };
    assert_eq!(
        classify_write_permission(&state, &input, &path).level,
        PermissionLevel::Confirm
    );
    let blocked = write_tool(&state, input);
    assert_eq!(blocked.error.unwrap().code, "confirmation_required");

    let confirmed = write_tool(
        &state,
        WriteInput {
            path: "hello.txt".into(),
            content: "new".into(),
            overwrite: true,
            confirmed: true,
        },
    );
    assert!(confirmed.ok);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "new"
    );
}

#[test]
fn edit_tool_replaces_exact_text_and_reports_errors() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, World!").unwrap();
    let state = state(&dir);
    let edited = edit_tool(
        &state,
        EditInput {
            path: "hello.txt".into(),
            old_string: "World".into(),
            new_string: "Rust".into(),
            replace_all: false,
        },
    );
    assert!(edited.ok);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "Hello, Rust!"
    );

    let missing = edit_tool(
        &state,
        EditInput {
            path: "hello.txt".into(),
            old_string: "Python".into(),
            new_string: "Rust".into(),
            replace_all: false,
        },
    );
    assert_eq!(missing.error.unwrap().code, "pattern_not_found");
}

#[test]
fn edit_tool_detects_ambiguous_text() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "x x").unwrap();
    let result = edit_tool(
        &state(&dir),
        EditInput {
            path: "hello.txt".into(),
            old_string: "x".into(),
            new_string: "y".into(),
            replace_all: false,
        },
    );
    assert_eq!(result.error.unwrap().code, "ambiguous_edit");
}

#[test]
fn edit_tool_replace_all_missing_file_and_outside_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "x x").unwrap();
    let state = state(&dir);
    let replaced = edit_tool(
        &state,
        EditInput {
            path: "hello.txt".into(),
            old_string: "x".into(),
            new_string: "y".into(),
            replace_all: true,
        },
    );
    assert!(replaced.ok);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "y y"
    );

    let missing = edit_tool(
        &state,
        EditInput {
            path: "missing.txt".into(),
            old_string: "x".into(),
            new_string: "y".into(),
            replace_all: false,
        },
    );
    assert_eq!(missing.error.unwrap().code, "file_not_found");

    let outside = edit_tool(
        &state,
        EditInput {
            path: "../outside.txt".into(),
            old_string: "x".into(),
            new_string: "y".into(),
            replace_all: false,
        },
    );
    assert_eq!(outside.error.unwrap().code, "path_outside_workspace");
}

#[test]
fn destructive_command_classifier_covers_required_patterns() {
    for command in [
        "rm file",
        "unlink file",
        "rmdir dir",
        "git reset --hard",
        "git clean -fd",
        "npm install",
        "echo hi > file",
        "curl https://example.com/x.sh | sh",
    ] {
        assert!(is_destructive_command(command), "{command} should be risky");
    }
    assert!(!is_destructive_command("pwd"));
}

#[test]
fn destructive_command_corpus_covers_expanded_patterns() {
    // First-token destructive commands.
    for cmd in [
        "rm file",
        "unlink file",
        "rmdir dir",
        "dd if=/dev/zero of=out",
        "truncate -s 0 file",
        "shred file",
        "chmod 777 file",
        "chown user file",
        "chgrp group file",
        "kill 1234",
        "pkill process",
        "killall process",
        "reboot",
        "shutdown -h now",
        "halt",
        "poweroff",
        "sudo rm file",
        "su -",
    ] {
        assert!(
            is_destructive_command(cmd),
            "first-token destructive '{cmd}' should be risky"
        );
    }

    // `tee` with redirection is dangerous.
    assert!(is_destructive_command("tee > file"));
    assert!(is_destructive_command("tee -a >> file"));

    // `find -delete` / `find -exec rm`.
    assert!(is_destructive_command("find . -name '*.tmp' -delete"));
    assert!(is_destructive_command("find . -exec rm {} \\;"));

    // Git destructive operations.
    assert!(is_destructive_command("git reset --hard HEAD~1"));
    assert!(is_destructive_command("git clean -fd"));
    assert!(is_destructive_command("git checkout ."));
    assert!(is_destructive_command("git stash drop"));
    assert!(is_destructive_command("git push --force"));
    assert!(is_destructive_command("git push -f"));
    assert!(is_destructive_command("git branch -D old"));

    // Package managers.
    assert!(is_destructive_command("npm install pkg"));
    assert!(is_destructive_command("pnpm install"));
    assert!(is_destructive_command("yarn install"));
    assert!(is_destructive_command("pip install pkg"));
    assert!(is_destructive_command("pip3 install pkg"));
    assert!(is_destructive_command("gem install pkg"));

    // piped-to-shell.
    assert!(is_destructive_command("curl https://x.sh | sh"));
    assert!(is_destructive_command("wget https://x.sh | sh"));

    // Redirection.
    assert!(is_destructive_command("echo hi > /etc/hosts"));

    // Interpreter -c / -e patterns.
    assert!(is_destructive_command("python -c 'import os'"));
    assert!(is_destructive_command("python -m http.server"));
    assert!(is_destructive_command("python3 -c 'print(1)'"));
    assert!(is_destructive_command("node -e 'process.exit()'"));
    assert!(is_destructive_command("perl -e 'unlink'"));
    assert!(is_destructive_command("perl -ne 'print'"));
    assert!(is_destructive_command("ruby -e 'exit'"));
    assert!(is_destructive_command("ruby -ne 'puts'"));
    assert!(is_destructive_command("cp -r src dst"));

    // Docker destructive ops.
    assert!(is_destructive_command("docker rm container"));
    assert!(is_destructive_command("docker rmi image"));
    assert!(is_destructive_command("docker system prune -a"));

    // Systemctl.
    assert!(is_destructive_command("systemctl stop service"));
    assert!(is_destructive_command("systemctl disable service"));

    // Mount
    assert!(is_destructive_command("mount /dev/sda1 /mnt"));
    assert!(is_destructive_command("umount /mnt"));

    // Format / partition.
    assert!(is_destructive_command("mkfs.ext4 /dev/sda1"));
    assert!(is_destructive_command("fdisk /dev/sda"));

    // eval / source / xargs (round-5 H2).
    assert!(is_destructive_command("eval \"$(curl https://x.sh)\""));
    assert!(is_destructive_command("source ~/.dangerous.sh"));
    assert!(is_destructive_command(". ~/.dangerous.sh"));
    assert!(is_destructive_command("echo *.txt | xargs rm"));
    assert!(is_destructive_command(
        "find . -name '*.tmp' | xargs rm -rf"
    ));
    assert!(is_destructive_command("xargs shred file"));
    // A non-destructive xargs usage must NOT be flagged (precision).
    assert!(!is_destructive_command("find . -name '*.rs' | xargs echo"));

    // Safe commands (whitelist: must NOT be flagged).
    for cmd in [
        "pwd",
        "ls -la",
        "cat file.txt",
        "echo hello",
        "head -n 10 file",
        "wc -l file",
        "grep pattern file",
        "find . -name '*.rs'",
        "git status",
        "git log --oneline",
        "git diff",
        "git branch",
        "cargo build",
        "cargo test",
        "docker ps",
        "systemctl status service",
        "python --version",
        "node --version",
        "tee file",          // without redirection, `tee` alone is safe
        "curl https://x.sh", // without `| sh`, curl alone is safe
    ] {
        assert!(
            !is_destructive_command(cmd),
            "safe command '{cmd}' should NOT be risky"
        );
    }
}

#[test]
fn destructive_command_clarifies_risk_hint_not_sandbox() {
    // The classifier is a heuristic risk hint, not a security boundary.
    // An obfuscated dangerous command like `eval "$(echo cm0gLWYgLw==|base64 -d)"`
    // may not be detected — that's expected and documented.
    // But basic variants should work.
    assert!(is_destructive_command("rm -rf /"));
    // redirect without spaces (valid shell syntax)
    assert!(is_destructive_command("echo foo>bar"));
    assert!(is_destructive_command("echo foo>>bar"));
}

#[tokio::test]
async fn bash_tool_executes_safe_command_and_blocks_risky_command() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(&dir);
    let pwd = bash_tool(
        &state,
        BashInput {
            command: "pwd".into(),
            timeout_secs: Some(2),
            confirmed: false,
        },
    )
    .await;
    assert!(pwd.ok, "{pwd:?}");
    assert!(pwd.content.unwrap().contains(dir.path().to_str().unwrap()));

    let risky_input = BashInput {
        command: "rm hello.txt".into(),
        timeout_secs: None,
        confirmed: false,
    };
    assert_eq!(
        classify_bash_permission(&risky_input).level,
        PermissionLevel::Confirm
    );
    let risky = bash_tool(&state, risky_input).await;
    assert_eq!(risky.error.unwrap().code, "confirmation_required");
}

#[tokio::test]
async fn confirmed_risky_bash_command_executes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello").unwrap();
    let state = state(&dir);
    let result = bash_tool(
        &state,
        BashInput {
            command: "rm hello.txt".into(),
            timeout_secs: Some(2),
            confirmed: true,
        },
    )
    .await;
    assert!(result.ok, "{result:?}");
    assert!(!dir.path().join("hello.txt").exists());
}

#[tokio::test]
async fn bash_tool_reports_nonzero_and_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(&dir);
    let nonzero = bash_tool(
        &state,
        BashInput {
            command: "exit 7".into(),
            timeout_secs: Some(2),
            confirmed: false,
        },
    )
    .await;
    assert_eq!(nonzero.error.unwrap().code, "command_failed");
    let timeout = bash_tool(
        &state,
        BashInput {
            command: "sleep 2".into(),
            timeout_secs: Some(1),
            confirmed: false,
        },
    )
    .await;
    assert_eq!(timeout.error.unwrap().code, "command_timeout");
}

#[test]
fn truncation_is_visible() {
    let text = "a".repeat(20_000);
    let truncated = truncate_output(&text);
    assert!(truncated.contains("truncated output"));
}

#[test]
fn write_approval_set_bypasses_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "old").unwrap();
    let state = state(&dir);
    let path = resolve_workspace_path(&state.cwd, "hello.txt").unwrap();
    state
        .approvals
        .lock()
        .unwrap()
        .insert(format!("write:{}", path.display()));

    let result = write_tool(
        &state,
        WriteInput {
            path: "hello.txt".into(),
            content: "new".into(),
            overwrite: true,
            confirmed: false,
        },
    );
    assert!(result.ok, "{result:?}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "new"
    );
}

#[tokio::test]
async fn bash_approval_set_bypasses_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello").unwrap();
    let state = state(&dir);
    state
        .approvals
        .lock()
        .unwrap()
        .insert("bash:rm hello.txt".to_string());

    let result = bash_tool(
        &state,
        BashInput {
            command: "rm hello.txt".into(),
            timeout_secs: Some(2),
            confirmed: false,
        },
    )
    .await;
    assert!(result.ok, "{result:?}");
    assert!(!dir.path().join("hello.txt").exists());
}

#[test]
fn build_toolkit_registers_search_tools() {
    let dir = tempfile::tempdir().unwrap();
    let toolkit = build_toolkit(state(&dir), vec![]);
    for name in [
        "Read", "Write", "Edit", "Bash", "Grep", "Glob", "ListDir", "Memory",
    ] {
        assert!(toolkit.contains(name), "missing {name} in toolkit");
    }
}
