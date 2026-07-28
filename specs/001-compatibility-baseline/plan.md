# Implementation Plan: AgentScope Compatibility Baseline

**Branch**: `001-compatibility-baseline` | **Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-compatibility-baseline/spec.md`

## Summary

本 Feature 建立 AgentScope 兼容性基线——通过脚本辅助的符号提取和人工标注，生成针对锁定 AgentScope 上游版本的结构化基线数据。产物为 9 个 JSON 文件 + 1 个方法论文档，不包含 Rust 代码，不设计 crate 结构，不实现任何框架功能。基线数据将作为后续所有 Feature 的兼容性需求输入。

## Technical Context

**Language/Version**: Python 3.10+（仅用于符号提取脚本）

**Primary Dependencies**: Python 标准库 `inspect`、`ast`、`importlib`；`pip` 或 `gh` CLI（获取包信息和克隆上游仓库）

**Storage**: 文件系统——JSON 文件存储在 `specs/001-compatibility-baseline/`

**Testing**: 本 Feature 无代码产物。质量验证通过 JSON schema 校验 + 人工交叉比对 AgentScope `__init__.py` 导出列表完成

**Target Platform**: 开发者工作站（Linux/macOS）

**Project Type**: 文档/基线数据产出

**Performance Goals**: 不适用

**Constraints**: 100-300 个能力条目；纯静态分析；一次性快照

**Scale/Scope**: 1 个 AgentScope release 版本的完整公开 API 清单，约 5-10 个顶层模块

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 名称 | 状态 | 说明 |
|------|------|------|------|
| 第一条 | 兼容性优先 | ✅ PASS | 本 Feature 为兼容性工作提供基础输入，不改变任何行为 |
| 第二条 | 锁定上游版本 | ✅ PASS | FR-001/FR-002 直接实现 |
| 第三条 | Python AgentScope 是行为基准 | ✅ PASS | 基于上游源码静态分析；后续 Feature 的运行时验证以本基线定义的 Trace 为标准 |
| 第四条 | 先定义契约，再实现代码 | ✅ PASS | 本 Feature 为后续所有模块提供 specification 输入 |
| 第五条 | 不允许伪兼容 | ✅ N/A | 不实现功能 |
| 第六条 | 测试驱动兼容性 | ✅ PASS | FR-013-015 定义 Trace 和归一化规则 |
| 第七条 | Trace 是核心验收产物 | ✅ PASS | FR-013 覆盖所有必需字段 |
| 第八条 | Rust 原生设计 | ✅ N/A | 无 Rust 代码 |
| 第九条 | 安全 Rust 优先 | ✅ N/A | 无 Rust 代码 |
| 第十条 | 结构化并发 | ✅ N/A | 无并发 |
| 第十一条 | 分层与依赖方向 | ✅ N/A | 无代码架构 |
| 第十二条 | 稳定的数据协议 | ✅ PASS | JSON schema 带版本号 |
| 第十三条 | 稳定错误模型 | ✅ N/A | 无运行时错误 |
| 第十四条 | 可观测性 | ✅ N/A | 无运行时 |
| 第十五条 | 性能不能牺牲正确性 | ✅ N/A | 无性能约束 |
| 第十六条 | 小步交付 | ✅ PASS | 宪法建议的能力拆分第 1 步 |
| 第十七条 | 完成的定义 | ✅ PASS | 完成标准由 SC-001 至 SC-014 定义 |
| 第十八条 | 兼容性分级 | ✅ PASS | FR-008 定义 L0-L5 等级 |
| 第十九条 | 变更治理 | ✅ N/A | 不修改宪法 |

**Gate Result**: ALL PASS — 无违反项。

## Project Structure

### Documentation (this feature)

```text
specs/001-compatibility-baseline/
├── spec.md                        # Feature specification
├── plan.md                        # This file
├── research.md                    # Phase 0: AgentScope upstream analysis
├── data-model.md                  # Phase 1: JSON data model definitions
├── quickstart.md                  # Phase 1: how to validate the baseline
├── contracts/                     # Phase 1: JSON schemas for each artifact
│   ├── version-lock.schema.json
│   ├── api-inventory.schema.json
│   ├── capability-matrix.schema.json
│   ├── dependency-map.schema.json
│   ├── example-inventory.schema.json
│   ├── trace-schema.schema.json
│   ├── normalization-rules.schema.json
│   └── exclusion-list.schema.json
├── tasks.md                       # Phase 2: /speckit-tasks output
├── checklists/
│   └── requirements.md            # Spec quality checklist
├── version-lock.json              # Deliverable
├── api-inventory.json             # Deliverable
├── capability-matrix.json         # Deliverable
├── dependency-map.json            # Deliverable
├── example-inventory.json         # Deliverable
├── trace-schema.json              # Deliverable
├── normalization-rules.json       # Deliverable
├── exclusion-list.json            # Deliverable
└── methodology.md                 # Deliverable
```

### Source Code (repository root)

```text
# 本 Feature 不产生任何源代码变更
# 所有产物位于 specs/001-compatibility-baseline/ 目录下
```

**Structure Decision**: 所有基线产物与 specification 文档共存于同一 feature 目录内。这符合 FR-018 要求，也使基线数据与 spec 紧密关联——后续 Feature 引用基线时只需按文件引用。

## Complexity Tracking

无宪法违反项，无需记录。
