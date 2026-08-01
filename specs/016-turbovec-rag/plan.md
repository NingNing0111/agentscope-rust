# Implementation Plan: Turbovec RAG 向量存储实现

**Branch**: `016-turbovec-rag` | **Date**: 2026-07-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/016-turbovec-rag/spec.md`

## Summary

实现 `TurbovecVectorStore` —— `VectorStore` trait 的第二个具体实现，基于 turbovec crate（Google TurboQuant 算法，2-4 bit/维度压缩，SIMD 加速搜索）。这是一个零外部数据库依赖的本地向量存储方案：将向量压缩编码后存储于内存中的 `IdMapIndex`，提供完整的 CRUD 操作和磁盘持久化。

## Technical Context

**Language/Version**: Rust 1.89+ (turbovec 的 MSRV；当前项目 workspace 使用稳定版 Rust)

**Primary Dependencies**: `turbovec` 0.9.x (crates.io), `tokio` (sync::RwLock, task::spawn_blocking), `serde`/`serde_json` (元数据序列化), `uuid` (document_id 生成)

**Storage**: 内存内 `turbovec::IdMapIndex`（压缩向量索引）+ 磁盘持久化（`.tvim` 文件 + JSON manifest）；无需外部数据库

**Testing**: `cargo test` — tokio async tests + turbovec sync 测试；`tempfile` 用于持久化测试场景

**Target Platform**: 64-bit Linux/macOS（x86_64/aarch64）；turbovec 要求 `target_pointer_width = "64"`，不支持 32-bit/WASM

**Project Type**: library crate（在 `agent_scope_rag` 中新增 `turbovec_store` 模块文件）

**Performance Goals**: 插入 1000 条 1536 维向量 <1s；search top-10 <5ms；save+load round-trip <0.5s

**Constraints**: 无 unsafe 代码（`agent_scope_rag` 已有 `#![deny(unsafe_code)]`）；turbovec 内部 unsafe 限于 SIMD 内核，外部暴露 safe API

**Scale/Scope**: 单个 store 管理数十个 collection，每个 collection 支持百万级向量

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | 宪法条款 | 评估 | 备注 |
|---|---------|------|------|
| 1 | 兼容性优先 | ✅ 通过 | VectorStore trait 的新实现，不修改公开 API；行为与 mock 实现兼容 |
| 2 | 锁定上游版本 | ✅ N/A | 不涉及 Python AgentScope 兼容 |
| 3 | Python 行为基准 | ✅ N/A | 无 Python 对应物；Rust 独有优化路径 |
| 4 | 先定义契约 | ✅ 通过 | spec.md 已批准，22 FR 可追溯 |
| 5 | 不允许伪兼容 | ✅ 通过 | 6 个 trait 方法完整实现，无占位 |
| 6 | 测试驱动 | ✅ 通过 | 确定性测试向量，不依赖真实 LLM |
| 7 | Trace 核心验收 | ✅ N/A | VectorStore 是数据层 |
| 8 | Rust 原生设计 | ✅ 通过 | trait 实现、RwLock、确定性哈希 |
| 9 | 错误不可忽略 | ✅ 通过 | turbovec 错误 → VectorStoreError |
| 10 | 异步优先 | ⚠️ 注意 | trait 是 async，turbovec 是 sync → spawn_blocking 桥接 |
| 11 | 无全局状态 | ✅ 通过 | 状态封装在实例内 |
| 12-19 | 全部 | ✅ 通过 | 详见 research.md |

**Gate Result**: ✅ 全部通过。异步桥接在 Phase 0 research 中详细分析。

## Project Structure

### Documentation (this feature)

```text
specs/016-turbovec-rag/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── turbovec_vector_store.md     # API contract
│   └── turbovec_persistence.md      # 持久化格式
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_rag/
├── Cargo.toml                        # + turbovec 依赖
├── src/
│   ├── lib.rs                        # + pub mod turbovec_store;
│   ├── vector_store.rs               # [不变]
│   ├── chunker.rs                    # [不变]
│   ├── parser.rs                     # [不变]
│   ├── knowledge_base.rs             # [不变]
│   ├── rag_middleware.rs             # [不变]
│   ├── error.rs                      # [不变]
│   └── turbovec_store.rs            # [新增]
└── tests/
    ├── vector_store_mock.rs          # [不变]
    └── turbovec_store_tests.rs      # [新增]
```

**Structure Decision**: 仅修改 `agent_scope_rag` crate。新增 1 个生产文件 + 1 个测试文件。不新增 crate，不修改 trait 定义。

## Complexity Tracking

> 无宪法违规，此节为空。
