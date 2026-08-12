use pi_rust::tools::{
    DEFAULT_GLOB_MAX_RESULTS, GlobInput, GrepInput, ListDirInput, ToolState, glob_tool, grep_tool,
    list_dir_tool,
};

fn tool_state(dir: &tempfile::TempDir) -> ToolState {
    ToolState::new(dir.path().canonicalize().unwrap(), 1)
}

#[test]
fn glob_preserves_relative_paths_recursive_zero_depth_and_ordering() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/sub")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
    std::fs::write(dir.path().join("src/sub/lib.rs"), "").unwrap();
    std::fs::write(dir.path().join("src/readme.md"), "").unwrap();

    let result = glob_tool(
        &tool_state(&dir),
        GlobInput {
            pattern: "src/**/*.rs".into(),
            path: None,
        },
    );

    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    assert_eq!(
        content.lines().collect::<Vec<_>>(),
        vec!["src/main.rs", "src/sub/lib.rs"]
    );
}

#[test]
fn glob_skips_hidden_entries_and_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
    std::fs::write(dir.path().join(".hidden/secret.rs"), "").unwrap();
    std::fs::write(dir.path().join("visible.rs"), "").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("visible.rs"), dir.path().join("linked.rs"))
        .unwrap();

    let result = glob_tool(
        &tool_state(&dir),
        GlobInput {
            pattern: "**/*.rs".into(),
            path: None,
        },
    );

    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    assert!(content.contains("visible.rs"), "{content}");
    assert!(!content.contains(".hidden"), "{content}");
    assert!(!content.contains("linked.rs"), "{content}");
}

#[test]
fn grep_remains_literal_and_skips_hidden_entries_and_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
    std::fs::write(dir.path().join("literal.txt"), "a.*b\naxb\n").unwrap();
    std::fs::write(dir.path().join(".hidden/secret.txt"), "a.*b\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        dir.path().join("literal.txt"),
        dir.path().join("linked.txt"),
    )
    .unwrap();

    let result = grep_tool(
        &tool_state(&dir),
        GrepInput {
            pattern: "a.*b".into(),
            path: None,
            case_insensitive: false,
            max_results: None,
        },
    );

    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    assert!(content.contains("literal.txt:1:a.*b"), "{content}");
    assert!(!content.contains("literal.txt:2:axb"), "{content}");
    assert!(!content.contains(".hidden"), "{content}");
    assert!(!content.contains("linked.txt"), "{content}");
}

#[test]
fn glob_respects_result_cap() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..(DEFAULT_GLOB_MAX_RESULTS + 25) {
        std::fs::write(dir.path().join(format!("file-{i:03}.rs")), "").unwrap();
    }

    let result = glob_tool(
        &tool_state(&dir),
        GlobInput {
            pattern: "*.rs".into(),
            path: None,
        },
    );

    assert!(result.ok, "{result:?}");
    assert_eq!(
        result.content.unwrap().lines().count(),
        DEFAULT_GLOB_MAX_RESULTS
    );
}

#[test]
fn list_dir_preserves_direct_sorted_output_and_symlink_skip() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("b_dir")).unwrap();
    std::fs::create_dir(dir.path().join("a_dir")).unwrap();
    std::fs::write(dir.path().join("b.txt"), "").unwrap();
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("a.txt"), dir.path().join("linked.txt")).unwrap();

    let result = list_dir_tool(
        &tool_state(&dir),
        ListDirInput {
            path: ".".into(),
            show_hidden: false,
        },
    );

    assert!(result.ok, "{result:?}");
    let content = result.content.unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines, vec!["a_dir/", "b_dir/", "a.txt", "b.txt"]);
}
