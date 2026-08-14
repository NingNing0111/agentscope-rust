# 镜像映射清单契约（Mirror Map Contract）

**目的**: 定义 `docs/rust/mirror-map.md` 的结构与维护规则，作为「docs/python 页面 ↔ docs/rust 页面 ↔ 实现状态 ↔ 引用示例」一比一对齐的权威依据（spec FR-011）。

## 1. 文件位置

`docs/rust/mirror-map.md`（与 `zh/` 平级，属文档元数据而非站点页面）。

## 2. 表结构

| 列 | 内容 |
|----|------|
| Python 页面 | `docs/python/zh/...` 相对路径（镜像源） |
| Rust 页面 | `docs/rust/zh/...` 相对路径（本期交付目标） |
| 实现状态 | `已实现` / `部分支持` / `计划中` |
| 兼容等级 | `L1`–`L4` / `—`（未实现页面留空） |
| 引用示例 | `examples/<name>` 或 `—` |
| 备注 | 关键偏差、版本差、openapi.json 例外等 |

## 3. 覆盖与一致性

- MUST 覆盖 `docs/python/zh` 的全部 50 个页面（en 侧 `deploy/openapi.json` 记录为显式例外）。
- 任一列更新时 MUST 同步更新文档页面状态块、兼容性矩阵与本表，三者保持一致。
- docs/python 新增/删除页面时，本表用于检测 docs/rust 的结构漂移（缺页或多页）。

## 4. 版本声明

本表头部 MUST 记录：

- 镜像源：`docs/python`（Mintlify，版本路径 `2.0.7dev`）
- Rust 兼容基线：AgentScope Python `v2.0.5`（commit `27b6a0d2a2afedf53462c9a2add33932d54b2d20`，见 CHANGELOG）
- 生成/更新日期

## 5. 例外登记

| 例外 | 原因 |
|------|------|
| `docs/python/en/deploy/openapi.json` 不镜像 | Python 后端 OpenAPI 生成物；Rust 当前无 agent-service 后端 |

其他任何缺页/多页 MUST 在此登记并附原因。
