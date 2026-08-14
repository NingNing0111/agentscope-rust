//! Skill example: load skills from a local directory with `LocalSkillLoader`.
//!
//! Needs no model or API key. A skills directory is a folder containing a
//! `SKILL.md` (YAML frontmatter + markdown body). This example creates one on
//! the fly, loads it, and prints the discovered skill's metadata.

use std::fs;

use agent_scope_tool::LocalSkillLoader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Build a temporary skills directory with one SKILL.md.
    let dir = tempfile::tempdir()?;
    let skill_dir = dir.path().join("summarize");
    fs::create_dir_all(&skill_dir)?;
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: summarize
description: Summarize long text into a short bullet list.
---

# Summarize

When the user asks to summarize, read the text and produce a concise bullet list.
"#,
    )?;

    // 2. Load skills from the directory.
    let loader = LocalSkillLoader::new(dir.path().to_str().unwrap(), true);
    let skills = loader.list_skills_blocking();

    println!("discovered {} skill(s):", skills.len());
    for skill in &skills {
        println!("  - {} | {}", skill.name, skill.description);
        println!("    dir: {}", skill.dir);
        println!("    markdown: {} bytes", skill.markdown.len());
    }

    assert!(
        !skills.is_empty(),
        "expected at least one skill to be discovered"
    );
    println!("\nOK: skills loaded from a local directory.");
    Ok(())
}
