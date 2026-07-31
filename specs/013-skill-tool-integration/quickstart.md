# Quickstart: Skill Tool Integration

**Feature**: 013-skill-tool-integration | **Date**: 2026-07-31

## Prerequisites

- Rust toolchain (nightly or stable 1.75+)
- Repo cloned at `agentscope-rust/`
- Workspace can build: `cargo check --workspace`

## Scenario 1: SkillViewer — Agent retrieves a skill

**Setup**: Create a temporary skill directory with a valid SKILL.md.

```bash
# Create a test skill
mkdir -p /tmp/test-skill
cat > /tmp/test-skill/SKILL.md << 'EOF'
---
name: example-skill
description: An example skill for testing
---

# Example Skill

This skill provides example functionality.

## Usage

1. Step one
2. Step two
EOF
```

**Run**: Build and run the test verifying SkillViewer call.

```bash
cargo test -p agent_scope_tool -- skill_viewer
```

**Expected outcome**:
- `test_skill_viewer_returns_markdown` PASS — SkillViewer returns the markdown body
- `test_skill_viewer_not_found` PASS — unknown skill returns error ToolChunk
- `test_skill_viewer_allows_all` PASS — permission check returns ALLOW

## Scenario 2: LocalSkillLoader — scan and cache skills

**Setup**: Create a directory with multiple skill subdirectories.

```bash
mkdir -p /tmp/skills-test/skill-a
cat > /tmp/skills-test/skill-a/SKILL.md << 'EOF'
---
name: skill-a
description: First test skill
---
# Skill A Content
EOF

mkdir -p /tmp/skills-test/skill-b
cat > /tmp/skills-test/skill-b/SKILL.md << 'EOF'
---
name: skill-b
description: Second test skill
---
# Skill B Content
EOF
```

**Run**:

```bash
cargo test -p agent_scope_tool -- skill_loader
```

**Expected outcome**:
- `test_local_loader_scan_subdir` PASS — returns 2 skills
- `test_local_loader_cache_hit` PASS — second scan uses cache
- `test_local_loader_missing_name_skipped` PASS — skill without name is skipped
- `test_local_loader_directory_not_exists` PASS — returns empty list

## Scenario 3: ToolKit skill registration and prompt generation

**Run**:

```bash
cargo test -p agent_scope_tool -- toolkit_skill
```

**Expected outcome**:
- `test_toolkit_add_skill_dir` PASS — skill registered and listable
- `test_toolkit_get_skill_instructions` PASS — prompt contains `<agent-skills>` XML
- `test_toolkit_get_skill_instructions_empty` PASS — returns empty string when no skills
- `test_toolkit_skill_dedup` PASS — duplicate names handled correctly
- `test_toolkit_skill_viewer_registered` PASS — SkillViewer appears in tool schemas

## Scenario 4: End-to-end — Agent invokes Skill tool

This scenario verifies the full chain: workspace → toolkit → Agent.

```bash
cargo test -p agent_scope_tool -- e2e_skill
```

**Expected outcome**: Agent successfully:
1. Receives system prompt containing available skill names
2. Calls `Skill` tool with a skill name
3. Receives skill markdown content
4. Can use the instructions from the markdown

## Scenario 5: LocalSkillLoader with 20 skills (Performance)

**Run**:

```bash
cargo test -p agent_scope_tool -- skill_loader_perf -- --nocapture
```

**Expected outcome**: Loading 20 skills completes in under 1 second (cold), under 200ms (warm cache).

## Running All Skill-Related Tests

```bash
# Run all tests in the skill modules
cargo test -p agent_scope_tool -- skill

# Run with output to see tracing messages
RUST_LOG=info cargo test -p agent_scope_tool -- skill -- --nocapture

# Verify clippy passes
cargo clippy -p agent_scope_tool -- -D warnings

# Verify workspace still green
cargo test --workspace
```

## Verification Checklist

- [ ] `cargo build` succeeds with no new warnings
- [ ] `cargo test --workspace` all tests pass
- [ ] `cargo clippy --workspace` 0 errors
- [ ] `cargo fmt --check --all` clean
- [ ] `SkillViewer` tool appears in `ToolKit::get_tool_schemas()` output
- [ ] `ToolKit::get_skill_instructions()` produces valid XML prompt fragment
- [ ] `LocalSkillLoader::list_skills()` handles: valid, invalid, missing, empty, cached paths
