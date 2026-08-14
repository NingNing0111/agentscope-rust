# AgentScope Rust 文档（docs/rust）

AgentScope Rust（`agent_scope_*` crates）的中文使用文档。目录结构 **一比一镜像** `docs/python`，便于在 Python 版与 Rust 版之间按相同路径对照阅读。

## 版本声明

- **镜像源**: `docs/python`（Mintlify，版本路径 `2.0.7dev`）
- **Rust 兼容基线**: AgentScope Python **v2.0.5**（commit `27b6a0d2a2afedf53462c9a2add33932d54b2d20`）
- **站内链接版本号**: `/versions/0.1.0/zh/...`（与 `Cargo.toml` `workspace.package.version` 一致）

> `docs/python` 为 2.0.7dev 文档，Rust 兼容基线锁定 v2.0.5。2.0.7dev 新增而 v2.0.5/Rust 未实现的能力，在对应页面标注「计划中」。**这是有意的诚实标注，不是文档缺陷**——它保证每个页面的内容都是真实可用的，而非伪造（宪法 §5）。

## 实现状态三档

每个页面顶部有 `<Note>` 状态块，取值为三档之一：

| 状态 | 含义 |
|------|------|
| **已实现** | 该能力在 AgentScope Rust 中可用，文档内容为真实 Rust 用法 |
| **部分支持** | 部分可用，页面列出「已支持 / 尚未实现」边界 |
| **计划中** | Rust 尚未实现，页面只描述 Python 侧能力与缺失范围，无 Rust 代码 |

状态标注规范见 [`STATUS-BLOCK.md`](STATUS-BLOCK.md)；逐页状态对照见 [`mirror-map.md`](mirror-map.md)。

## 内容导航

- **[index](zh/index.mdx)** — 索引入口，能力总览
- **[quickstart](zh/quickstart.mdx)** — 快速上手，30 分钟跑起第一个 Agent
- **building-blocks/** — 各能力模块：agent / tool / model / memory / rag / workspace / sandbox / middleware / permission-system / context / message-and-event / plan
- **deploy/** — 服务化部署能力（多数为「计划中」）
- **others/** — change-log、faq
- **[release-notes](zh/release-notes.mdx)** — 版本历史

## 示例

文档中的代码示例存放在仓库根 `examples/` 下，均为 workspace 成员 crate，可独立编译运行（`cargo run -p <name>`）：

| 示例 crate | 演示能力 | 文档页 |
|-----------|----------|--------|
| `examples/quickstart` | 最小 Agent、reply/reply_stream | index、quickstart |
| `examples/chat` | 流式对话、事件分发 | message-and-event、agent/run-agent |
| `examples/tool` | FunctionTool、ToolKit、内置工具 | tool/* |
| `examples/mcp` | MCP 客户端接入 | tool/mcp |
| `examples/skill` | Skill 工具 | tool/skill |
| `examples/agent` | 编排、权限、中断、任务工具 | agent/*、middleware、permission-system、plan |
| `examples/memory` | 长期记忆双后端 | long-term-memory |
| `examples/rag` | 检索增强问答 | rag |
| `examples/workspace` | LocalWorkspace、内置工具注入 | workspace/* |
| `examples/sandbox` | 沙箱命令执行、路径防护 | workspace/overview、sandbox |

真实模型调用需要环境变量 `DASHSCOPE_API_KEY`；无凭据时示例会给出明确错误提示。
