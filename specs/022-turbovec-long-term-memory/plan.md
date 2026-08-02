# Implementation Plan: TurboVec Long-Term Memory

**Branch**: `022-turbovec-long-term-memory` | **Date**: 2026-08-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/022-turbovec-long-term-memory/spec.md`

## Summary

在 `agent_scope_memory` crate 中新增 `TurbovecMemory` 实现——`Memory` trait 的第二个具体实现，组合 `FileMemory`（Markdown 持久化）+ `EmbeddingModel`（文本向量化）+ `TurbovecVectorStore`（高性能语义检索），以双层架构提供基于 turbovec 的长期记忆存储和语义检索能力。Markdown 文件是 source of truth，TurboVec 向量索引是可重建的派生数据。

## Technical Context

**Language/Version**: Rust 2024 edition (workspace), 64-bit target required

**Primary Dependencies**: `agent_scope_memory` (Memory trait, FileMemory, Backend), `agent_scope_embedding` (EmbeddingModel), `agent_scope_rag` (TurbovecVectorStore), `turbovec` 0.9.x (transitive via rag crate), `tokio` (sync::RwLock, task::spawn_blocking)

**Storage**: Markdown files (via FileMemory delegate) + `{memory_dir}/.turbovec/` 持久化向量索引 (`.tvim` + `.meta` + `manifest.json`)

**Testing**: `cargo test -p agent_scope_memory` — tokio async tests + turbovec blocking tests via spawn_blocking; mock `EmbeddingModel` for deterministic tests; `tempfile` for persistence round-trip

**Target Platform**: 64-bit Linux/macOS (x86_64/aarch64); WASM/32-bit unsupported (turbovec requirement)

**Project Type**: library crate — 在 `agent_scope_memory` 中新增 `turbovec_memory` 模块文件

**Performance Goals**: 1000 条 memory 的 top-10 语义搜索 <100ms（不含 embedding 生成）；rebuild 1000 条 <10s（不含 embedding 生成）

**Constraints**: 无 unsafe 代码（`#![deny(unsafe_code)]` 已启用）；trait object 扩展模式（`Arc<dyn EmbeddingModel>`）；不修改 `Memory` trait 签名

**Scale/Scope**: 单进程本地，支持数千条 memory，单个 TurboVec collection

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | 宪法条款 | 评估 | 备注 |
|---|---------|------|------|
| 1 | 兼容性优先 | ✅ 通过 | Memory trait 的新实现，不修改公开 API；与 FileMemory 行为兼容 |
| 2 | 锁定上游版本 | ✅ N/A | 不涉及 Python AgentScope 兼容 |
| 3 | Python 行为基准 | ✅ N/A | Rust 独有优化路径；FileMemory 兼容性由 Feature 009 保证 |
| 4 | 先定义契约 | ✅ 通过 | spec.md 已批准，25 FR，contracts/ 4 files |
| 5 | 不允许伪兼容 | ✅ 通过 | Memory trait 7 methods 完整实现 or delegate |
| 6 | 测试驱动 | ✅ 通过 | 确定性 mock EmbeddingModel 测试，不依赖真实 LLM |
| 7 | Trace 核心验收 | ✅ 通过 | write/delete/search/rebuild 均含 tracing span |
| 8 | Rust 原生设计 | ✅ 通过 | struct + trait impl + Arc<dyn> 组合模式 |
| 9 | 安全 Rust 优先 | ✅ 通过 | `#![deny(unsafe_code)]`，turbovec 内部 unsafe 限于 SIMD 且外部 safe |
| 10 | 结构化并发 | ✅ 通过 | spawn_blocking 桥接 turbovec sync ops |
| 11 | 分层依赖 | ✅ 通过 | `agent_scope_memory → agent_scope_rag + agent_scope_embedding` 无循环 |
| 12 | 稳定数据协议 | ✅ 通过 | MemoryEntry/MemoryType 复用；manifest.json 有 version 字段，unknown fields 兼容 |
| 13 | 稳定错误模型 | ✅ 通过 | `MemoryError::SemanticIndexError` 新变体，typed error |
| 14 | 可观测性 | ✅ 通过 | tracing instrument on write/delete/search/rebuild |
| 15 | 性能不牺牲正确性 | ✅ 通过 | TurboVec 是派生索引，Markdown 是 source of truth；索引丢失可重建 |
| 16 | 小步交付 | ✅ 通过 | 独立 feature：新增一个 Memory trait 实现 |
| 17 | 完成的定义 | ✅ 通过 | plan + tasks + tests + docs + clippy + fmt |
| 18 | 兼容性分级 | ✅ L2 | 核心行为兼容：Memory trait 行为与 FileMemory 一致，语义检索行为是新能力 |
| 19 | 变更治理 | ✅ N/A | 无宪法违反 |

**Gate Result**: ✅ 全部通过。

## Project Structure

### Documentation (this feature)

```text
specs/022-turbovec-long-term-memory/
├── plan.md                          # This file
├── research.md                      # Phase 0 output
├── data-model.md                    # Phase 1 output
├── quickstart.md                    # Phase 1 output
├── contracts/                       # Phase 1 output
│   ├── turbovec-memory.md           # Core struct + Memory trait impl contract
│   ├── semantic-index.md            # MemoryEntry → VectorRecord mapping contract
│   ├── index-persistence.md         # Persistence format contract
│   └── rebuild-and-consistency.md   # Rebuild + consistency contract
└── tasks.md                         # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_memory/
├── Cargo.toml                        # + agent_scope_embedding, agent_scope_rag deps
├── src/
│   ├── lib.rs                        # + pub mod turbovec_memory;
│   ├── memory_trait.rs               # [不变]
│   ├── memory_entry.rs               # [不变]
│   ├── memory_config.rs              # [不变] or + TurbovecMemoryConfig re-export
│   ├── memory_error.rs               # + SemanticIndexError variant
│   ├── file_memory.rs                # [不变]
│   ├── backend.rs                    # [不变]
│   ├── frontmatter.rs                # [不变]
│   ├── index.rs                      # [不变]
│   ├── retrieval.rs                  # [不变]
│   └── turbovec_memory.rs            # [新增] — TurbovecMemory + TurbovecMemoryConfig
└── tests/
    ├── file_memory_tests.rs          # [不变]
    ├── retrieval_tests.rs            # [不变]
    └── turbovec_memory_tests.rs      # [新增]
```

**Structure Decision**: 仅修改 `agent_scope_memory` crate。新增 1 个生产文件 (`turbovec_memory.rs`) + 1 个测试文件 (`turbovec_memory_tests.rs`)。Cargo.toml 新增 2 个 workspace 依赖 (`agent_scope_embedding`, `agent_scope_rag`)。不新增 crate，不修改 `Memory` trait 签名。`MemoryError` 新增 `SemanticIndexError` 变体。

## Complexity Tracking

> 无宪法违规，此节为空。
