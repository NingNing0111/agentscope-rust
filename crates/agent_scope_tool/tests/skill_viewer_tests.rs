//! Tests for SkillViewer Tool (US1) + ToolKit skill integration (US2) + prompt generation (US3).

use std::collections::HashMap;

use agent_scope_message::{ToolCallBlock, ToolOutput, ToolResultState};
use agent_scope_tool::{SkillViewer, Tool, ToolExecOutput, ToolKit};
use agent_scope_workspace::Skill;

// ============================================================================
// US1 Tests: SkillViewer (T010, T011, T012)
// ============================================================================

fn make_skill_viewer_with_skills(skills_map: HashMap<String, Skill>) -> SkillViewer {
    SkillViewer::new(Box::new(move |_groups| skills_map.clone()))
}

#[tokio::test]
async fn test_skill_viewer_returns_markdown_for_known_skill() {
    // T010
    let mut map = HashMap::new();
    map.insert(
        "test".to_string(),
        Skill {
            name: "test".into(),
            description: "A test skill".into(),
            dir: "/tmp/test-skill".into(),
            markdown: "# Hello".into(),
            updated_at: 0.0,
        },
    );
    let viewer = make_skill_viewer_with_skills(map);

    let result = viewer
        .call(serde_json::json!({"skill": "test"}))
        .await
        .unwrap();

    match result {
        ToolExecOutput::Complete(chunk) => {
            assert_eq!(chunk.state, ToolResultState::Success);
            match &chunk.output {
                ToolOutput::Text(text) => assert_eq!(text, "# Hello"),
                _ => panic!("Expected Text output"),
            }
        }
        _ => panic!("Expected Complete"),
    }
}

#[tokio::test]
async fn test_skill_viewer_returns_error_for_unknown_skill() {
    // T011
    let viewer = make_skill_viewer_with_skills(HashMap::new());

    let result = viewer
        .call(serde_json::json!({"skill": "unknown"}))
        .await
        .unwrap();

    match result {
        ToolExecOutput::Complete(chunk) => {
            assert_eq!(chunk.state, ToolResultState::Error);
            match &chunk.output {
                ToolOutput::Text(text) => {
                    assert!(
                        text.contains("SkillNotFoundError"),
                        "expected SkillNotFoundError, got: {text}"
                    );
                    assert!(text.contains("unknown"));
                }
                _ => panic!("Expected Text output"),
            }
        }
        _ => panic!("Expected Complete"),
    }
}

#[tokio::test]
async fn test_skill_viewer_callback_panic_is_caught() {
    // T012
    let viewer = SkillViewer::new(Box::new(|_groups| {
        panic!("simulated callback panic");
    }));

    let result = viewer
        .call(serde_json::json!({"skill": "anything"}))
        .await
        .unwrap();

    match result {
        ToolExecOutput::Complete(chunk) => {
            assert_eq!(chunk.state, ToolResultState::Error);
            match &chunk.output {
                ToolOutput::Text(text) => {
                    assert!(text.contains("SkillNotFoundError"));
                    assert!(text.contains("internal error"));
                }
                _ => panic!("Expected Text output"),
            }
        }
        _ => panic!("Expected Complete"),
    }
}

#[tokio::test]
async fn test_skill_viewer_missing_skill_field() {
    // T012 extension: no "skill" key in input
    let viewer = make_skill_viewer_with_skills(HashMap::new());

    let result = viewer.call(serde_json::json!({})).await.unwrap();

    match result {
        ToolExecOutput::Complete(chunk) => {
            assert_eq!(chunk.state, ToolResultState::Error);
            match &chunk.output {
                ToolOutput::Text(text) => {
                    assert!(text.contains("SkillNotFoundError"));
                }
                _ => panic!("Expected Text output"),
            }
        }
        _ => panic!("Expected Complete"),
    }
}

// ============================================================================
// US2 Tests: ToolKit skill registration (T018, T019, T020, T021, T022)
// ============================================================================

#[tokio::test]
async fn test_toolkit_add_skill_dir_registers_skill() {
    // T018
    let dir = tempfile::tempdir().unwrap();
    let skill_dir = dir.path().join("my-skill");
    std::fs::create_dir(&skill_dir).unwrap();

    let skill_md_content =
        "---\nname: my-skill\ndescription: A test skill from dir\n---\n\n# My Skill";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md_content).unwrap();

    let mut tk = ToolKit::new();
    tk.add_skill_dir(skill_dir.to_str().unwrap());

    let skills = tk.list_skills().await;
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "my-skill");
    assert!(skills[0].markdown.contains("# My Skill"));

    let instructions = tk.get_skill_instructions(None);
    assert!(
        instructions.contains("my-skill"),
        "sync prompt must include add_skill_dir skills: {instructions}"
    );
    assert!(
        instructions.contains("A test skill from dir"),
        "sync prompt must include add_skill_dir descriptions: {instructions}"
    );

    let result = tk
        .call_tool(&ToolCallBlock::new(
            "call-skill".into(),
            "Skill".into(),
            r#"{"skill":"my-skill"}"#.into(),
        ))
        .await
        .unwrap();
    match result {
        ToolExecOutput::Complete(chunk) => {
            assert_eq!(chunk.state, ToolResultState::Success);
            match &chunk.output {
                ToolOutput::Text(text) => assert!(
                    text.contains("# My Skill"),
                    "Skill tool must read the same add_skill_dir skill listed in prompt: {text}"
                ),
                _ => panic!("Expected Text output"),
            }
        }
        _ => panic!("Expected Complete"),
    }
}

#[tokio::test]
async fn test_toolkit_add_skill_dir_missing_skill_md() {
    // T019: add empty dir — should silently return empty skills
    let dir = tempfile::tempdir().unwrap();
    let empty_dir = dir.path().join("empty-skill");
    std::fs::create_dir(&empty_dir).unwrap();

    let mut tk = ToolKit::new();
    tk.add_skill_dir(empty_dir.to_str().unwrap());

    let skills = tk.list_skills().await;
    assert!(skills.is_empty());
}

#[tokio::test]
async fn test_toolkit_add_two_skills_from_different_dirs() {
    // T020
    let dir = tempfile::tempdir().unwrap();

    let skill_a = dir.path().join("skill-a");
    std::fs::create_dir(&skill_a).unwrap();
    std::fs::write(
        skill_a.join("SKILL.md"),
        "---\nname: skill-a\ndescription: First skill\n---\n\n# A",
    )
    .unwrap();

    let skill_b = dir.path().join("skill-b");
    std::fs::create_dir(&skill_b).unwrap();
    std::fs::write(
        skill_b.join("SKILL.md"),
        "---\nname: skill-b\ndescription: Second skill\n---\n\n# B",
    )
    .unwrap();

    let mut tk = ToolKit::new();
    tk.add_skill_dir(skill_a.to_str().unwrap());
    tk.add_skill_dir(skill_b.to_str().unwrap());

    let skills = tk.list_skills().await;
    assert_eq!(skills.len(), 2);
    let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"skill-a"));
    assert!(names.contains(&"skill-b"));
}

#[tokio::test]
async fn test_toolkit_skill_viewer_auto_registered() {
    // T021
    let tk = ToolKit::new();
    let schemas = tk.get_tool_schemas();

    let has_skill_tool = schemas
        .iter()
        .any(|s| s["function"]["name"].as_str() == Some("Skill"));
    assert!(has_skill_tool, "SkillViewer tool should be auto-registered");
}

#[tokio::test]
async fn test_toolkit_duplicate_skill_name_dedup() {
    // T022
    let mut tk = ToolKit::new();
    tk.add_skill(Skill {
        name: "dup".into(),
        description: "First".into(),
        dir: "/tmp/a".into(),
        markdown: "A".into(),
        updated_at: 0.0,
    });
    tk.add_skill(Skill {
        name: "dup".into(),
        description: "Second".into(),
        dir: "/tmp/b".into(),
        markdown: "B".into(),
        updated_at: 0.0,
    });

    let skills = tk.list_skills().await;
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].description, "First"); // first wins
    assert!(tk.get_skill_instructions(None).contains("First"));
    assert!(!tk.get_skill_instructions(None).contains("Second"));
}

// ============================================================================
// US3 Tests: System prompt generation (T040, T041, T042)
// ============================================================================

#[tokio::test]
async fn test_get_skill_instructions_with_registered_skills() {
    // T040
    let mut tk = ToolKit::new();
    tk.add_skill(Skill {
        name: "example-skill".into(),
        description: "An example skill".into(),
        dir: "/tmp/example".into(),
        markdown: "# Example".into(),
        updated_at: 0.0,
    });
    tk.add_skill(Skill {
        name: "another-skill".into(),
        description: "Another skill".into(),
        dir: "/tmp/another".into(),
        markdown: "# Another".into(),
        updated_at: 0.0,
    });

    let instructions = tk.get_skill_instructions(None);

    assert!(
        instructions.contains("<agent-skills>"),
        "missing <agent-skills>: {instructions}"
    );
    assert!(
        instructions.contains("example-skill"),
        "missing example-skill: {instructions}"
    );
    assert!(
        instructions.contains("another-skill"),
        "missing another-skill: {instructions}"
    );
    assert!(
        instructions.contains("An example skill"),
        "missing description"
    );
    assert!(instructions.contains("<name>"), "missing <name> tags");
    assert!(
        instructions.contains("<description>"),
        "missing <description> tags"
    );
    assert!(instructions.contains("<dir>"), "missing <dir> tags");
    assert!(
        !instructions.contains("{skill_viewer}"),
        "unreplaced placeholder"
    );
    assert!(
        !instructions.contains("{skills_list}"),
        "unreplaced placeholder"
    );
}

#[tokio::test]
async fn test_get_skill_instructions_empty_when_no_skills() {
    // T041
    let tk = ToolKit::new();
    let instructions = tk.get_skill_instructions(None);
    assert!(
        instructions.is_empty(),
        "expected empty string, got: {instructions}"
    );
}

#[tokio::test]
async fn test_get_skill_instructions_with_custom_template() {
    // T042
    let mut tk = ToolKit::new();
    tk.add_skill(Skill {
        name: "custom-skill".into(),
        description: "A custom skill".into(),
        dir: "/tmp/custom".into(),
        markdown: "# Custom".into(),
        updated_at: 0.0,
    });

    let custom_template = "CUSTOM_START\n{skills_list}\nCUSTOM_END";
    let instructions = tk.get_skill_instructions(Some(custom_template));

    assert!(instructions.contains("CUSTOM_START"));
    assert!(instructions.contains("CUSTOM_END"));
    assert!(instructions.contains("custom-skill"));
}
