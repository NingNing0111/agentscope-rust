# Contract: Skill 工具（SkillViewer）

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_skill.py`（上游 `9d1026fa`）+ Rust `agent_scope_tool/src/skill_viewer.rs`（既有）

## 工具名

`Skill`（实现类 `SkillViewer`，与既有 Rust 实现一致）

## 描述（对齐 Python `_skill.py:24` 与既有 Rust `skill_viewer.rs:82`）

在会话中检索技能。当用户要求执行任务时，检查是否有可用技能匹配。技能提供专业能力和领域知识。

## 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "skill": {
      "type": "string",
      "description": "The exact name of the skill to view."
    }
  },
  "required": ["skill"]
}
```

## 行为契约（对齐 Python `_skill.py:90`）

| 步骤 | 行为 |
|------|------|
| 参数校验 | 缺 `skill` → 错误 `SkillNotFoundError: missing required 'skill' parameter`（既有 Rust 行为） |
| 技能解析 | 通过 `get_skills_method(activated_groups)` 取激活组的技能（对齐 Python `_skill.py:112`） |
| 精确匹配 | 精确名称查找；大小写不匹配/近似名 → 未找到 |
| 未找到 | 返回 `SkillNotFoundError: Skill '{skill}' not found.`（Error） |
| 命中 | 返回技能 markdown 内容（Success） |

## 激活组交互

- Python `SkillViewer` 按 `_agent_state.tool_context.activated_groups` 过滤技能（`_skill.py:112`）。
- Rust 既有 `SkillViewer` 用 `ListSkillsCallback(&[String])` 回调接收激活组（`skill_viewer.rs:24`）。
- 029 需确保 callback 传入的激活组来自 `AgentState.tool_context.activated_groups`（与 ResetTools 一致）。

## 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| 缺 `skill` 参数 | `ValidationFailure` | `invalid_arguments` |
| 技能不存在 | `ValidationFailure` | `skill_not_found` |

## 元属性

`is_read_only = true` | `is_concurrency_safe = true` | `is_state_injected = true`

## 交叉引用

- Python: `_skill.py`
- 既有: `agent_scope_tool/src/skill_viewer.rs`（`SkillViewer` 已注册于 `ToolKit::new()`）
- Spec: FR-020~021
