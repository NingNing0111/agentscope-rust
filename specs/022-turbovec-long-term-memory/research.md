# Research: TurboVec Long-Term Memory

**Feature**: 022-turbovec-long-term-memory | **Date**: 2026-08-02

## 1. 架构决策：双层存储模型

**Decision**: Markdown 文件作为 source of truth + TurboVec 向量索引作为可重建派生索引。

**Rationale**:
- Feature 009 `FileMemory` 已明确 Markdown + frontmatter 是长期记忆的持久化格式，具备人类可读、可外部编辑、Python AgentScope 兼容等目标
- 将记忆体全部迁移到 `.tvim` 二进制格式会违背 Feature 009 的设计意图
- TurboVec 仅加速语义检索，不替代文件存储
- 索引丢失或损坏时可从 Markdown 文件全量重建

**Alternatives considered**:
- 纯 TurboVec 存储（`.tvim` 作为唯一副本）：违反 Feature 009 可读性目标，拒绝
- 仅增强现有 LLM-based `retrieve_relevant()`：未利用 turbovec 的高性能向量检索，达不到 Feature 022 的 turbovec 约束要求

## 2. 技术选型：组合现有组件

**Decision**: 在 `agent_scope_memory` crate 中新增实现，组合 `FileMemory` + `EmbeddingModel` + `TurbovecVectorStore`。

**Rationale**:
- `agent_scope_memory::Memory` trait 已稳定（write/read/delete/list/search/retrieve_relevant）
- `agent_scope_embedding::EmbeddingModel` 提供 embedding 生成能力
- `agent_scope_rag::TurbovecVectorStore` 提供高性能本地向量存储
- 新增实现不必修改 trait 签名

**Dependency direction check** (Constitution §11):
- `agent_scope_memory` 当前依赖: `agent_scope_message`, `agent_scope_model`
- 需要在 `agent_scope_memory` 中新增依赖 `agent_scope_embedding`, `agent_scope_rag`
- 这引入了 `agent_scope_memory → agent_scope_rag` 依赖边
- `agent_scope_rag` 当前不依赖 `agent_scope_memory`（无循环）
- `agent_scope_rag` 依赖 `agent_scope_embedding`（已有）
- **结论**: 依赖方向合规，不会产生循环

**Alternatives considered**:
- 在 `agent_scope_rag` 中实现：语义不对——rag 层不应理解 Memory/MemoryEntry 概念
- 新建独立 crate：过度工程化，仅为一个实现增加 crate

## 3. `retrieve_relevant()` 增强策略

**Decision**: 语义检索（EmbeddingModel + TurboVec）替代 LLM 文件选择器，ChatModel 仅用于最终结果 rerank/格式化（可选）。

**Rationale**:
- 现有 Feature 009 `retrieve_relevant()` 使用 ChatModel 结构化输出选择文件，依赖 LLM 判断
- Feature 022 的核心价值是用向量检索实现确定性、高性能的语义相关性匹配
- 保留 ChatModel 参数维持 trait 兼容性，实现内部可降级为仅做结果截断/格式化

**Implementation approach**:
1. 将用户 query 通过 `EmbeddingModel::embed()` 转为向量
2. 在 TurboVec 中搜索 top-k 相似 chunk
3. 按 memory name 去重，整理为 `MemorySearchResult`
4. 截断到 `max_results` 限制，格式化返回

**Alternatives considered**:
- 完全保留 LLM selector + TurboVec 仅作为候选预过滤：增加 LLM 延迟，违背 Feature 022 高性能目标
- 修改 trait 签名移除 ChatModel 参数：破坏向后兼容，拒绝

## 4. 持久化策略

**Decision**: 在 `{memory_dir}/.turbovec/` 下持久化向量索引，与 Markdown 文件同生命周期。

**Rationale**:
- `.turbovec/` 前缀避免污染 memory 文件列表
- 通过 `list()` 自动排除
- 便于删除/重建
- 复用 `TurbovecVectorStore::save/load` 的 atom write + manifest

**Directory layout**:
```text
{memory_dir}/
├── MEMORY.md
├── user-role.md
├── project-note.md
└── .turbovec/
    ├── manifest.json
    ├── memories.tvim
    └── memories.meta
```

## 5. Memory Entry → Vector Record 映射

**Decision**: 每个 `MemoryEntry` 作为一个 document（`document_id = memory_name`），整体生成一个 embedding vector。

**Rationale**:
- Memory entry 通常较短（前几百字），不需要 chunking
- 简化 upsert/delete 语义：每次 write 删除旧 document_id 全部记录，插入新记录
- 若未来需要支持超长 memory，可扩展为 chunking

**Metadata 映射**:
| VectorRecord metadata key | MemoryEntry 来源 |
|---------------------------|-----------------|
| `memory_name` | `entry.name` |
| `memory_type` | `entry.metadata.mem_type.as_str()` |
| `source` | `"{name}.md"` |
| `updated_at` | `entry.metadata.updated_at` |

## 6. 错误处理策略

**Decision**: 继承 `MemoryError` + 明确类型化错误，`retrieve_relevant()` 失败降级为 `Ok(None)`。

**Rationale**:
- Feature 009 已定义 `MemoryError` 枚举，新增变体 `SemanticIndexError { reason: String }` 
- `retrieve_relevant()` 的模型/embedding 失败不阻断 agent 循环（Constitution §13 fail-open）
- embedding model 错误不应 panic

**New error variant**: `MemoryError::SemanticIndexError { reason: String }` — 向量索引层错误（index corruption, dimension mismatch, rebuild needed）

## 7. bit_width 选择

**Decision**: 默认 `bit_width = 4`，可配置。

**Rationale**:
- Feature 016 基准：bit_width=4 在 recall 上最接近原始向量
- 长期记忆通常条目数远小于 RAG 文档量，不需要极致压缩
- 2/3 bit_width 作为可选项保留

## 8. 平台约束

**Decision**: 继承 Feature 016 的平台声明：64-bit Linux/macOS，WASM/32-bit 不支持。

**Rationale**: turbovec crate 要求 `target_pointer_width = "64"`，底层 SIMD 汇编仅在 x86_64/aarch64 可用。
