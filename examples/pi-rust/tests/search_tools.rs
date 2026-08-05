//! Tests for the Grep / Glob / ListDir search tools and the shared approval
//! fingerprint helper.

use pi_rust::tools::{
    GlobInput, GrepInput, ListDirInput, ToolState, approval_fingerprint, glob_to_regex, glob_tool,
    grep_tool, list_dir_tool,
};
use regex::Regex;

fn tool_state(dir: &tempfile::TempDir) -> ToolState {
    ToolState::new(dir.path().canonicalize().unwrap(), 1)
}

#[test]
fn approval_fingerprint_bash_and_write() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path();
    let bash = serde_json::json!({ "command": "  rm hello.txt  " });
    assert_eq!(
        approval_fingerprint("Bash", &bash, cwd),
        Some("bash:rm hello.txt".into())
    );
    let write = serde_json::json!({ "path": "hello.txt" });
    let fp = approval_fingerprint("Write", &write, cwd).unwrap();
    assert!(fp.starts_with("write:"), "{fp}");
    assert!(fp.ends_with("/hello.txt"), "{fp}");
    // Non-gated tools have no fingerprint.
    assert_eq!(approval_fingerprint("Read", &bash, cwd), None);
}

#[test]
fn grep_finds_matching_lines_with_file_line_numbers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.txt"),
        "hello world\nfoo bar\nhello again\n",
    )
    .unwrap();
    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "hello".into(),
            path: None,
            case_insensitive: false,
            max_results: None,
        },
    );
    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    assert!(content.contains("a.txt:1:hello world"), "{content}");
    assert!(content.contains("a.txt:3:hello again"), "{content}");
}

#[test]
fn grep_skips_hidden_and_binary() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
    std::fs::write(dir.path().join(".hidden/x.txt"), "secret pattern\n").unwrap();
    std::fs::write(dir.path().join("bin.dat"), [0xff, 0xfe, b's', b'e']).unwrap();
    std::fs::write(dir.path().join("visible.txt"), "secret pattern\n").unwrap();
    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "secret".into(),
            path: None,
            case_insensitive: false,
            max_results: None,
        },
    );
    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    assert!(content.contains("visible.txt"), "{content}");
    assert!(!content.contains(".hidden"), "{content}");
    assert!(!content.contains("bin.dat"), "{content}");
}

#[test]
fn grep_is_case_insensitive_when_requested() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "HELLO WORLD\n").unwrap();
    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "hello".into(),
            path: None,
            case_insensitive: true,
            max_results: None,
        },
    );
    assert!(result.ok, "{result:?}");
    assert!(result.content.unwrap().contains("a.txt:1:HELLO WORLD"));
}

#[test]
fn grep_respects_max_results() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = String::new();
    for i in 0..10 {
        content.push_str(&format!("line {i} needle\n"));
    }
    std::fs::write(dir.path().join("a.txt"), content).unwrap();
    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "needle".into(),
            path: None,
            case_insensitive: false,
            max_results: Some(3),
        },
    );
    assert!(result.ok, "{result:?}");
    assert!(result.content.unwrap().lines().count() <= 3);
}

#[test]
fn grep_empty_pattern_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "".into(),
            path: None,
            case_insensitive: false,
            max_results: None,
        },
    );
    assert_eq!(result.error.unwrap().code, "invalid_arguments");
}

#[test]
fn glob_matches_recursive_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/sub")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
    std::fs::write(dir.path().join("src/sub/lib.rs"), "").unwrap();
    std::fs::write(dir.path().join("README.md"), "").unwrap();
    let result = glob_tool(
        &tool_state(&dir),
        GlobInput {
            pattern: "src/**/*.rs".into(),
            path: None,
        },
    );
    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    assert!(content.contains("src/main.rs"), "{content}");
    assert!(content.contains("src/sub/lib.rs"), "{content}");
    assert!(!content.contains("README.md"), "{content}");
}

#[test]
fn glob_no_match_returns_ok_without_content() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "").unwrap();
    let result = glob_tool(
        &tool_state(&dir),
        GlobInput {
            pattern: "**/*.xyz".into(),
            path: None,
        },
    );
    assert!(result.ok, "{result:?}");
    assert!(result.content.is_none());
}

#[test]
fn list_dir_lists_sorted_dirs_first() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("zzz")).unwrap();
    std::fs::write(dir.path().join("aaa.txt"), "").unwrap();
    std::fs::write(dir.path().join("mmm"), "").unwrap();
    let result = list_dir_tool(
        &tool_state(&dir),
        ListDirInput {
            path: ".".into(),
            show_hidden: false,
        },
    );
    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    let entries: Vec<&str> = content.lines().collect();
    // Directories sort before files; within a group, alphabetical.
    assert_eq!(entries[0], "zzz/", "{content}");
    assert!(entries.contains(&"aaa.txt"), "{content}");
    assert!(entries.contains(&"mmm"), "{content}");
}

#[test]
fn list_dir_hides_dotfiles_unless_requested() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".env"), "").unwrap();
    std::fs::write(dir.path().join("visible"), "").unwrap();
    let hidden_off = list_dir_tool(
        &tool_state(&dir),
        ListDirInput {
            path: ".".into(),
            show_hidden: false,
        },
    );
    assert!(!hidden_off.content.unwrap().contains(".env"));
    let hidden_on = list_dir_tool(
        &tool_state(&dir),
        ListDirInput {
            path: ".".into(),
            show_hidden: true,
        },
    );
    assert!(hidden_on.content.unwrap().contains(".env"));
}

#[test]
fn new_tools_reject_outside_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let state = tool_state(&dir);
    let grep = grep_tool(
        &state,
        GrepInput {
            pattern: "x".into(),
            path: Some("../x".into()),
            case_insensitive: false,
            max_results: None,
        },
    );
    assert_eq!(grep.error.unwrap().code, "path_outside_workspace");
    let glob = glob_tool(
        &state,
        GlobInput {
            pattern: "*".into(),
            path: Some("../x".into()),
        },
    );
    assert_eq!(glob.error.unwrap().code, "path_outside_workspace");
    let list = list_dir_tool(
        &state,
        ListDirInput {
            path: "../x".into(),
            show_hidden: false,
        },
    );
    assert_eq!(list.error.unwrap().code, "path_outside_workspace");
}

#[test]
fn glob_to_regex_matches_common_patterns() {
    let anchored = |pattern: &str| Regex::new(&format!("^{}$", glob_to_regex(pattern))).unwrap();
    let root_rs = anchored("*.rs");
    assert!(root_rs.is_match("main.rs"));
    assert!(!root_rs.is_match("sub/main.rs"));

    let any_depth_rs = anchored("**/*.rs");
    assert!(any_depth_rs.is_match("main.rs"));
    assert!(any_depth_rs.is_match("a/b/main.rs"));

    let src_any_rs = anchored("src/**/*.rs");
    assert!(src_any_rs.is_match("src/main.rs"));
    assert!(src_any_rs.is_match("src/a/b.rs"));

    let single = anchored("a?c.txt");
    assert!(single.is_match("abc.txt"));
    assert!(!single.is_match("ac.txt"));
}
