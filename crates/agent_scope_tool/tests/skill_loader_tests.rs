//! Tests for LocalSkillLoader (US4) — edge cases: scanning, caching, error handling.

use agent_scope_tool::{LocalSkillLoader, SkillLoader};

#[test]
fn test_local_loader_blocking_matches_async_loader_results() {
    let dir = tempfile::tempdir().unwrap();

    let sub_a = dir.path().join("skill-a");
    std::fs::create_dir(&sub_a).unwrap();
    create_skill_md(&sub_a, "skill-a", "First skill", "# A Content");

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);
    let blocking = loader.list_skills_blocking();
    let async_skills = futures::executor::block_on(loader.list_skills());

    assert_eq!(blocking.len(), async_skills.len());
    assert_eq!(blocking[0].name, async_skills[0].name);
    assert_eq!(blocking[0].description, async_skills[0].description);
    assert_eq!(blocking[0].markdown, async_skills[0].markdown);
}

/// Helper to create a SKILL.md file in a directory.
fn create_skill_md(dir: &std::path::Path, name: &str, description: &str, body: &str) {
    let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}");
    std::fs::write(dir.join("SKILL.md"), content).unwrap();
}

#[tokio::test]
async fn test_local_loader_scan_subdir_true_loads_from_subdirs() {
    // T030
    let dir = tempfile::tempdir().unwrap();

    let sub_a = dir.path().join("skill-a");
    std::fs::create_dir(&sub_a).unwrap();
    create_skill_md(&sub_a, "skill-a", "First skill", "# A Content");

    let sub_b = dir.path().join("skill-b");
    std::fs::create_dir(&sub_b).unwrap();
    create_skill_md(&sub_b, "skill-b", "Second skill", "# B Content");

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);
    let skills = loader.list_skills().await;

    assert_eq!(skills.len(), 2);
    let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"skill-a"));
    assert!(names.contains(&"skill-b"));
}

#[tokio::test]
async fn test_local_loader_cache_second_scan_uses_cache() {
    // T031: modified file re-read, unchanged files use cache
    let dir = tempfile::tempdir().unwrap();

    let sub = dir.path().join("skill-x");
    std::fs::create_dir(&sub).unwrap();
    create_skill_md(&sub, "skill-x", "Test skill", "# Original");

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);

    // First scan
    let skills1 = loader.list_skills().await;
    assert_eq!(skills1.len(), 1);
    assert_eq!(skills1[0].markdown, "# Original");
    let mtime1 = skills1[0].updated_at;

    // Modify the file
    std::thread::sleep(std::time::Duration::from_millis(10)); // ensure mtime changes
    let content = "---\nname: skill-x\ndescription: Updated skill\n---\n\n# Modified";
    std::fs::write(sub.join("SKILL.md"), content).unwrap();

    // Second scan — should detect mtime change and re-read
    let skills2 = loader.list_skills().await;
    assert_eq!(skills2.len(), 1);
    assert_eq!(skills2[0].markdown, "# Modified");
    assert!(skills2[0].updated_at > mtime1);
}

#[tokio::test]
async fn test_local_loader_scan_subdir_false_only_checks_root() {
    // T032
    let dir = tempfile::tempdir().unwrap();

    // Root SKILL.md
    create_skill_md(dir.path(), "root-skill", "Root skill", "# Root");

    // Subdir SKILL.md
    let sub = dir.path().join("nested");
    std::fs::create_dir(&sub).unwrap();
    create_skill_md(&sub, "nested-skill", "Nested skill", "# Nested");

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), false);
    let skills = loader.list_skills().await;

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "root-skill");
}

#[tokio::test]
async fn test_local_loader_missing_name_is_skipped() {
    // T033: SKILL.md with description but no name → skipped
    let dir = tempfile::tempdir().unwrap();

    let sub = dir.path().join("bad-skill");
    std::fs::create_dir(&sub).unwrap();
    let content = "---\ndescription: Has description but no name\n---\n\n# No Name";
    std::fs::write(sub.join("SKILL.md"), content).unwrap();

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);
    let skills = loader.list_skills().await;

    assert!(skills.is_empty(), "skill without name should be skipped");
}

#[tokio::test]
async fn test_local_loader_directory_not_exists_returns_empty() {
    // T034
    let loader = LocalSkillLoader::new("/nonexistent/path/12345", true);
    let skills = loader.list_skills().await;
    assert!(skills.is_empty());
}

#[tokio::test]
async fn test_local_loader_malformed_frontmatter_gracefully_skipped() {
    // T035: SKILL.md with frontmatter missing both required fields → skipped
    let dir = tempfile::tempdir().unwrap();

    let sub = dir.path().join("malformed");
    std::fs::create_dir(&sub).unwrap();
    // Frontmatter with invalid content — no name or description field
    let content = "---\nfoo: bar\nbaz: qux\n---\n\nJust body with no name/description";
    std::fs::write(sub.join("SKILL.md"), content).unwrap();

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);
    let skills = loader.list_skills().await;

    assert!(
        skills.is_empty(),
        "frontmatter without name/description should be skipped"
    );
}

#[tokio::test]
async fn test_local_loader_empty_markdown_body_is_accepted() {
    // T036: frontmatter with empty body → accepted
    let dir = tempfile::tempdir().unwrap();

    let sub = dir.path().join("empty-body");
    std::fs::create_dir(&sub).unwrap();
    let content = "---\nname: empty-body\ndescription: No body\n---\n";
    std::fs::write(sub.join("SKILL.md"), content).unwrap();

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);
    let skills = loader.list_skills().await;

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "empty-body");
    assert!(skills[0].markdown.is_empty());
}

#[tokio::test]
async fn test_local_loader_missing_description_is_skipped() {
    // T033 extension: missing description also skipped
    let dir = tempfile::tempdir().unwrap();

    let sub = dir.path().join("no-desc");
    std::fs::create_dir(&sub).unwrap();
    let content = "---\nname: no-desc\n---\n\n# Has name but no description";
    std::fs::write(sub.join("SKILL.md"), content).unwrap();

    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);
    let skills = loader.list_skills().await;

    assert!(
        skills.is_empty(),
        "skill without description should be skipped"
    );
}

#[tokio::test]
async fn test_local_loader_not_a_directory_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("not-a-dir.txt");
    std::fs::write(&file_path, "hello").unwrap();

    let loader = LocalSkillLoader::new(file_path.to_str().unwrap(), true);
    let skills = loader.list_skills().await;
    assert!(skills.is_empty());
}
