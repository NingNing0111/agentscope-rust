---
title: "Skill"
description: "用 Markdown 指令集拓展智能体能力"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。技能加载与查看在 AgentScope Rust 中可用。兼容基线为 AgentScope Python v2.0.5。
</Note>

Skill（技能）是 Markdown 格式的指令集，无需写代码即可拓展智能体能力。每个 skill 是一个目录，固定包含一个带 frontmatter 元数据与详细指令的 `SKILL.md` 文件。

与工具不同，skill 不能被直接调用。智能体通过 Skill 查看器读取 skill 指令，再用现有工具按指令执行。

## 核心类型（`agent_scope_tool`）

| 类型 | 职责 |
|------|------|
| `SkillLoader` / `LocalSkillLoader` | 从本地目录发现并解析技能（`LocalSkillLoader::new(dir, scan_subdir)`） |
| `SkillViewer` | 按名称查询技能内容，供 Skill 工具使用 |
| `SkillTool` | 内置 Skill 工具：运行时按精确名称读取技能 |
| `Skill`（`agent_scope_workspace`） | 技能数据：`name` / `description` / `dir` / `markdown` / `updated_at` |

## 加载技能

```rust
use agent_scope_tool::LocalSkillLoader;

let loader = LocalSkillLoader::new("/path/to/skills", true);
let skills = loader.list_skills_blocking();
for skill in &skills {
    println!("- {} | {}", skill.name, skill.description);
}
```

技能目录可直接放入 workspace 的技能目录，由 `SkillManager`（`agent_scope_workspace`）管理；`SKILL.md` 支持 YAML frontmatter（含块标量多行描述）。

## 完整示例

见 [`examples/skill`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/skill/)（`cargo run -p skill`），临时创建一个 `SKILL.md` 并用 `LocalSkillLoader` 加载，无需模型凭据。
