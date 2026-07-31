# Research: Workspace Management

**Feature**: 012-workspace-management | **Date**: 2026-07-31

## 1. Backend 抽象层设计

### Decision
创建 `WorkspaceBackend` trait，在现有 `agent_scope_memory::Backend` trait 基础上扩展，增加 `exec_shell`、`is_dir`、`delete_path`（递归删除）、`basename`、`dirname` 方法。`LocalBackend` 重新实现在 `agent_scope_workspace` crate 中（避免循环依赖）。

### Rationale
- 现有 `agent_scope_memory::Backend` 缺少 `exec_shell`、`is_dir`、`delete_path`（递归删除目录）、`basename`、`dirname`
- 直接扩展 `agent_scope_memory::Backend` 需要对 agent_scope_memory crate 做 breaking change，且该 trait 的 `list_dir` 签名使用 `Vec<String>`（全路径）而我们需要相对路径选项
- 在新的 crate 中定义 `WorkspaceBackend` 保持各 crate 边界清晰，未来 Sandbox/Docker/E2B 后端可直接实现此 trait

### Alternatives Considered
1. **直接扩展 `agent_scope_memory::Backend`**: 拒绝 — 该 trait 放在 memory crate 中语义不合理（Backend 不仅用于 memory）
2. **在 `agent_scope_tool` 中定义**: 拒绝 — tool crate 当前没有 Backend trait；Workspace 需要 Backend，Backend 不需要 tool
3. **新 trait 完全独立**: 采纳 — 放在 `agent_scope_workspace` crate 中，所有权清晰

## 2. WorkspaceBase trait 设计

### Decision
`WorkspaceBase` 定义为 `async_trait`，所有方法返回 `Result<T, WorkspaceError>`。不提供 Drop-based RAII（Rust async 析构没有等价于 Python `__aexit__` 的机制），改为要求调用者显式调用 `close()` 并提供文档说明。

### Rationale
- Rust 的 `Drop` trait 是同步的，无法执行异步清理（关闭 MCP 连接需要 `.await`）
- Python 的 `async with` 在 Rust 中没有直接等价物
- 提供 `close()` 方法并在文档中强调生命周期管理，符合 Rust 社区惯例（参考 `tokio::sync::Mutex` 的显式 `lock()`）

### Alternatives Considered
1. **使用 `Drop` + `block_on`**: 拒绝 — `block_on` 在 async 上下文中可能导致死锁
2. **实现 `Future` 并返回 guard 对象**: 过度设计 — 增加了 API 复杂度，收益有限
3. **显式 `close()` + 文档**: 采纳 — 简洁、安全、符合项目现有模式

## 3. MCP 客户端抽象

### Decision
在 workspace crate 中定义 `McpClientConfig` 结构体（name + transport config 的 JSON 序列化数据），而非依赖 `agent_scope_memory` 中的真实 MCP 客户端。`LocalWorkspace` 仅管理 MCP 配置的持久化和 CRUD，实际的 MCP 连接由上层调用者管理。

### Rationale
- 当前 `agent_scope_memory` 中没有 `MCPClient` 类型
- Workspace 的 MCP 管理本质上是配置注册表，不是 MCP 协议的实现者
- 将 MCP 客户端配置定义为 `serde`-serializable 的 struct，与 Python 的 `.mcp` 文件格式保持兼容
- 上层 Agent 框架负责从配置创建实际的 MCP 连接

### Alternatives Considered
1. **定义完整的 MCPClient trait**: 拒绝 — MCP 不在 workspace 范围内，过度耦合
2. **仅存储原始 JSON Value**: 拒绝 — 丢失类型安全
3. **定义 `McpClientConfig` struct**: 采纳 — 类型安全 + 序列化兼容

## 4. Skill 管理

### Decision
在 workspace crate 中定义 `Skill` struct 和 `SkillManager`。`Skill` 包含 name、description、dir、markdown、updated_at 字段。使用 `SkillManager` 负责 SKILL.md 解析、哈希去重、名称冲突解决、`.skills` 索引文件管理。

### Rationale
- Python 版 `.skills` 索引文件机制对性能关键（避免每次 list 都全量解析 SKILL.md）
- 使用 SHA-256 哈希去重保证幂等性
- `SkillManager` 封装所有 skill 操作，保持 `LocalWorkspace` 代码简洁

### Alternatives Considered
1. **不使用 `.skills` 索引**: 拒绝 — 每次 `list_skills()` 都 may 解析数百个文件，性能差
2. **使用文件系统 mtime 检测**: 采纳 — 与 Python 实现一致，仅当 mtime 变化时重建索引
3. **使用数据库（SQLite）**: 拒绝 — workspace 是文件系统原语，引入数据库过度复杂

## 5. Offload 文件格式

### Decision
`offload_context` 输出 JSONL 文件（每行一条 JSON 序列化的 `Msg`），`offload_tool_result` 输出纯文本 `.txt` 文件。Base64 数据块提取到 `data/` 目录，文件名 = `{sha256_of_base64}.{ext}`。

### Rationale
- JSONL 格式允许追加写入（O(1) 追加 vs O(n) 重写整个 JSON 数组）
- 文件名用 base64 内容的 SHA-256 哈希（而非解码后字节的哈希）与 Python 实现一致
- 纯文本格式的工具结果便于 Agent 通过 Read 工具检索

### Alternatives Considered
1. **使用 JSON 数组文件**: 拒绝 — 每次追加需要重写整个文件，不适合长会话
2. **使用 msgpack/bincode**: 拒绝 — 不可人类读取，与 Skill/Tool 工具的交互性差
3. **JSONL + data/ 提取**: 采纳 — 与 Python 实现一致，追加友好，可检索

## 6. WorkspaceManager 设计

### Decision
`WorkspaceManager` 使用 `Arc<RwLock<HashMap<String, ManagerEntry>>>` 管理 workspace 实例，每个 entry 包含 `Arc<dyn WorkspaceBase>` 和 `last_access: Instant`。提供一个后台清理任务（通过 `tokio::spawn`），每隔 TTL/2 检查并 evict 超时条目。

### Rationale
- `Arc<dyn WorkspaceBase>` 允许共享所有权，支持租约续期
- `RwLock` 因为读（get）远多于写（evict/insert）
- 后台清理任务是可选的：如果 TTL 为 `None`（永不淘汰），则不启动清理任务

### Alternatives Considered
1. **使用 `tokio::sync::Mutex`**: 拒绝 — 多读单写场景用 RwLock 性能更好
2. **使用 moka/quick_cache**: 过度设计 — 引入新依赖不必要，HashMap + TTL 足够
3. **无后台任务，惰性淘汰**: 可接受 — 但惰性淘汰可能造成长时间不用的 workspace 堆叠资源

## 7. 路径安全性

### Decision
所有 workspace 内部操作 MUST 验证目标路径在 `workdir` 范围内：使用 `std::fs::canonicalize` 后检查前缀匹配。Skill 复制和 offload 写入均需路径遍历保护。

### Rationale
- `canonicalize` 解析所有符号链接、`.` 和 `..`，之后 `starts_with` 检查保证安全性
- 与 Python 实现的 `os.path.realpath` + `.startswith` 模式一致

### Alternatives Considered
1. **仅用字符串前缀检查**: 拒绝 — 不解析符号链接，存在逃逸风险
2. **使用 `cap-std` crate**: 过度设计 — 引入不熟悉的依赖，团队学习成本高
3. **`canonicalize` + `starts_with`**: 采纳 — 标准库，无需新依赖，与 Python 一致
