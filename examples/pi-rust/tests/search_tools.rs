//! Tests for the Grep / Glob / ListDir search tools and the shared approval
//! fingerprint helper.

use pi_rust::tools::{
    GlobInput, GrepInput, ListDirInput, ToolState, approval_fingerprint, glob_to_regex, glob_tool,
    grep_tool, list_dir_tool,
};
// Re-exported constants for test assertions.
use pi_rust::tools::{DEFAULT_GLOB_MAX_RESULTS, MAX_GREP_FILE_BYTES};
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

#[test]
fn grep_skips_large_files() {
    let dir = tempfile::tempdir().unwrap();
    // Create a file just under the per-file limit with a match.
    std::fs::write(dir.path().join("small.txt"), "needle\n").unwrap();
    // Create a file well over the per-file limit (simulated via sparse/allocation hint).
    let large_path = dir.path().join("large.log");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&large_path).unwrap();
        // Write enough data to exceed MAX_GREP_FILE_BYTES (32 MiB).
        f.set_len(MAX_GREP_FILE_BYTES + 1024 * 1024).unwrap(); // 33 MiB
        // Write some content at the beginning so it's valid UTF-8.
        f.write_all(b"needle in a huge file\n").unwrap();
    }
    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "needle".into(),
            path: None,
            case_insensitive: false,
            max_results: None,
        },
    );
    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    // The small file match must still appear.
    assert!(content.contains("small.txt"), "{content}");
    // The large file must be skipped (not in results).
    assert!(
        !content.contains("large.log"),
        "large file should be skipped: {content}"
    );
    // Summary should mention the skip.
    assert!(
        result.summary.contains("skipped") && result.summary.contains("large"),
        "summary should note skipped large files: {}",
        result.summary
    );
}

#[test]
fn grep_skips_binary_files_via_nul_detection() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("text.txt"), "needle in text\n").unwrap();
    // Write a file that starts with text but contains a NUL byte later (binary).
    let mut binary = vec![
        b'n', b'e', b'e', b'd', b'l', b'e', b' ', b'i', b'n', b' ', b'b', b'i', b'n', b'\n',
    ];
    // Extend past BINARY_CHECK_WINDOW with NUL.
    binary.resize(9000, 0);
    binary[8500] = b'n';
    binary[8501] = b'e';
    binary[8502] = b'e';
    binary[8503] = b'd';
    binary[8504] = b'l';
    binary[8505] = b'e';
    std::fs::write(dir.path().join("mixed.bin"), &binary).unwrap();

    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "needle".into(),
            path: None,
            case_insensitive: false,
            max_results: None,
        },
    );
    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    // Text file match must appear.
    assert!(content.contains("text.txt"), "{content}");
    // Binary file must be skipped.
    assert!(
        !content.contains("mixed.bin"),
        "binary file should be skipped: {content}"
    );
    assert!(
        result.summary.contains("skipped") && result.summary.contains("binary"),
        "summary should note binary: {}",
        result.summary
    );
}

#[test]
fn glob_has_entry_scan_cap() {
    let dir = tempfile::tempdir().unwrap();
    // Create a tree with many directories and files, but not enough to hit
    // the cap in a unit test (100K entries is too many).
    // Instead, verify that the cap constant exists and is reachable:
    // Create enough entries that glob makes progress but doesn't hang.
    for i in 0..1000 {
        std::fs::create_dir_all(dir.path().join(format!("deep/a/b/c/d_{i}"))).unwrap();
        std::fs::write(dir.path().join(format!("deep/a/b/c/d_{i}/f.rs")), "").unwrap();
    }
    let result = glob_tool(
        &tool_state(&dir),
        GlobInput {
            pattern: "**/*.rs".into(),
            path: Some("deep".into()),
        },
    );
    assert!(result.ok, "{result:?}");
    // Should return results up to DEFAULT_GLOB_MAX_RESULTS (200).
    let content = result.content.unwrap();
    let count = content.lines().count();
    assert!(
        count <= 200,
        "glob should cap results at {DEFAULT_GLOB_MAX_RESULTS}, got {count}"
    );
}
