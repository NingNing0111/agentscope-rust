# 示例 crate 契约（Example Crate Contract）

**目的**: 定义 `examples/` 下每个示例 crate 必须满足的约束，使示例成为文档正确性的可编译锚点，同时保持 CI 轻量。

## 1. 注册与编译

- 每个示例是独立 crate，位于 `examples/<name>/`，登记在根 `Cargo.toml` 的 `[workspace] members`（与 `examples/pi-rust` 同模式）。
- MUST 通过 `cargo check --workspace --all-targets` 与 `cargo clippy --workspace --all-targets -D warnings`。
- 依赖复用 workspace 已有依赖；新依赖仅在有充分理由时引入，避免拖慢 CI。

## 2. 命名与目录

本期 10 个示例 crate：

| crate 名 | 文档页面 | 核心演示 |
|----------|----------|----------|
| `quickstart` | index / quickstart | 最小 Agent：凭据 + ChatModel + Toolkit + `reply`/`reply_stream` |
| `chat` | message-and-event / agent/run-agent | 流式对话 + 事件流分发（`EventType` match） |
| `tool` | tool/python-tool / tool/overview | `FunctionTool` 自定义工具 + `ToolKit` + 内置工具 |
| `mcp` | tool/mcp | MCP stdio server 连接 + `McpTool` 调用 |
| `skill` | tool/skill | `SkillLoader` + Skill 工具读取技能 |
| `agent` | agent/* / middleware / permission-system | Agent 编排 + 权限/人工确认 + 中断恢复 |
| `memory` | long-term-memory / memory | `FileMemory` / `TurbovecMemory` 读写与检索 |
| `rag` | rag | `KnowledgeBase` + `RAGMiddleware`（Static/Agentic） |
| `workspace` | workspace/* | `LocalWorkspace` 文件操作 + 内置工具注入 |
| `sandbox` | sandbox / workspace/overview | `SandboxSession` 命令执行 + 路径防护 |

## 3. 运行约束

- 真实模型调用依赖凭据（环境变量 `API_KEY`/`DASHSCOPE_API_KEY`）。
- 凭据缺失时，程序 MUST 给出明确、可操作的错误提示（`error: 缺少环境变量 DASHSCOPE_API_KEY，请 …`），而非静默失败或 panic。
- 不依赖凭据的路径（如本地工具、记忆文件、sandbox 本地执行）在无凭据时也应可独立演示。
- 每个示例提供 `--help` 或等价用法说明；README 注明运行命令与预期输出。

## 4. 代码质量

- `#![deny(unsafe_code)]`（宪法 §9）：示例不得使用 `unsafe`。
- 错误处理用 typed error / `anyhow` 传播，禁止对用户输入 `unwrap()`/`expect()` panic（宪法 §13）。
- 演示遵循 Rust 原生 API 形态（`Arc<dyn ChatModel>` 等，宪法 §8）。
- 结构化并发（宪法 §10）：tokio task 有明确生命周期，`Ctrl+C` 优雅取消。

## 5. 与文档的绑定

- 每篇引用示例的文档页 MUST 指向真实 crate 路径与运行命令。
- 示例的公共行为（工具 schema 输出、事件类型名、消息结构）MUST 与文档描述一致。
- 示例中出现的 crate/API 名即文档所宣称的公开 API，作为文档正确性的编译锚点。
