# Implementation Plan: Skill Tool Integration

**Branch**: `013-skill-tool-integration` | **Date**: 2026-07-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/013-skill-tool-integration/spec.md`

## Summary

在 `agent_scope_tool` crate 中实现 Skill Tool 集成层：新增 `SkillViewer` 工具（Agent 通过工具调用获取 skill markdown）、`SkillLoader` trait + `LocalSkillLoader`（从文件系统扫描 SKILL.md）、`ToolKit` 扩展（skill 注册/查询/prompt 生成）。与现有 `agent_scope_workspace::Skill` 类型无缝对接。

## Technical Context

**Language/Version**: Rust 2021 edition (same as workspace)

**Primary Dependencies**: `agent_scope_tool` (Tool trait), `agent_scope_workspace` (Skill struct), `serde`/`serde_json`, `serde_yaml` or `serde_yml` (SKILL.md frontmatter), `sha2` (hash 去重), `tracing`, `tokio`

**Storage**: 文件系统（SKILL.md 文件读取），ToolKit 内存 HashMap

**Testing**: `cargo test` with temp dirs for LocalSkillLoader tests, mock callbacks for SkillViewer tests

**Target Platform**: macOS/Linux (cross-platform), server-side

**Project Type**: library crate (`agent_scope_tool` 扩展)

**Performance Goals**: <100ms 端到端 SkillViewer 调用（不含 I/O），<1s 加载 20 个 skill 目录

**Constraints**: 不改变现有 `Tool` trait 公开 API，不改变 `WorkspaceBase` trait

**Scale/Scope**: 4 个新增类型/struct + 2 个 trait + ~500 行生产代码 + ~300 行测试

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Article | Status | Notes |
|---|---------|--------|-------|
| 1 | 兼容性优先 | ✅ PASS | SkillViewer 输入/输出与 Python 完全对齐；LocalSkillLoader 对标 Python `LocalSkillLoader` |
| 2 | 锁定上游版本 | ✅ PASS | Python 参考实现已在 `agentscope/` 目录中，同一 git commit |
| 3 | Python 是行为基准 | ✅ PASS | 差分测试将基于 Python `SkillViewer.call()` 和 `LocalSkillLoader.list_skills()` 的实际输出 |
| 4 | 先定义契约 | ✅ PASS | Spec 已创建 (spec.md)，本 plan 生成 contracts/ |
| 5 | 不允许伪兼容 | ✅ PASS | SkillNotFound 返回明确错误 ToolChunk，不静默忽略 |
| 6 | 测试驱动兼容性 | ✅ PASS | 测试策略：Mock callback SkillViewer 测试 + temp dir LocalSkillLoader 测试 |
| 7 | Trace 是核心验收 | ✅ PASS | SkillViewer 的 ToolChunk output 将纳入 trace 比较 |
| 8 | Rust 原生设计 | ✅ PASS | `SkillLoader` trait + `Box<dyn SkillLoader>` trait object，不模仿 Python 继承 |
| 9 | 安全 Rust 优先 | ✅ PASS | `#![deny(unsafe_code)]` 持续性；`unwrap` 仅测试代码使用 |
| 10 | 结构化并发 | ✅ PASS | `LocalSkillLoader` 使用 `tokio::join_all`，无自由 spawn |
| 11 | 分层与依赖方向 | ✅ PASS | Tool crate 依赖 workspace crate（Skill 类型），单一方向，无循环 |
| 12 | 稳定数据协议 | ✅ PASS | `Skill` struct 已有 `#[derive(Serialize, Deserialize)]`；新增类型同理 |
| 13 | 稳定错误模型 | ✅ PASS | 使用已有 `ToolError` variants，新增 `ToolError::SkillNotFound` variant |
| 14 | 可观测性 | ✅ PASS | 所有关键路径有 `tracing::info!/warn!` 日志 |
| 15 | 性能不能牺牲正确性 | ✅ PASS | 兼容性测试优先于性能；缓存优化在正确性验证后进行 |
| 16 | 编译时安全性 | ✅ PASS | 不引入新的 proc macro 或 build script 复杂性 |
| 17 | Crate 边界清晰 | ✅ PASS | 仅扩展 `agent_scope_tool`，不创建新 crate |
| 18 | 版本稳定性 | ✅ PASS | 新增为 public API 的增加性变更，不破坏现有 API |
| 19 | 文档完备 | ✅ PASS | lib.rs 文档 + quickstart.md + 各 struct 文档注释 |

**Gate result**: ALL 19 PASS — 无违规，无需 Complexity Tracking。

## Project Structure

### Documentation (this feature)

```text
specs/013-skill-tool-integration/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
├── spec.md              # Feature spec (input)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_tool/
├── src/
│   ├── lib.rs              # +pub mod skill_loader; +pub mod skill_viewer;
│   ├── tool_trait.rs       # +ToolError::SkillNotFound variant (+1 variant)
│   ├── toolkit.rs          # +skill 字段, +get_skill_instructions(), +add_skill_*()
│   ├── function.rs         # (no change)
│   ├── skill_loader.rs     # NEW: SkillLoader trait + LocalSkillLoader
│   └── skill_viewer.rs     # NEW: SkillViewer Tool implementation
└── tests/
    ├── skill_loader_tests.rs   # NEW: LocalSkillLoader 测试
    └── skill_viewer_tests.rs   # NEW: SkillViewer + ToolKit skill 集成测试
```

**依赖变更** (`agent_scope_tool/Cargo.toml`):
- 新增: `agent_scope_workspace` (for `Skill` type)
- 新增: `serde_yaml` or `serde_yml` (for SKILL.md frontmatter)
- 新增: `sha2` (for content hash, or reuse from workspace)

**Structure Decision**: 将 Skill 集成代码全部放在 `agent_scope_tool` crate 中（而非创建新 crate），因为 SkillViewer 是 Tool 的实现，LocalSkillLoader 天然属于 tool crate 的 skill 子模块。这与 Python agentscope 的模块组织一致（`tool/_builtin/_skill.py` 和 `skill/_local_loader.py` 在 repo 中但功能上绑定）。

## Complexity Tracking

> No violations — this section intentionally left empty.
