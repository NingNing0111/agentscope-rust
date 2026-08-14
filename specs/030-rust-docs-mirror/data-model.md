# 数据模型：Feature 030 文档站点

**日期**: 2026-08-13
**范围**: 文档站（docs/rust）、示例 crate（examples/）、镜像映射清单（mirror-map）三类产物的实体与关系。本 feature 无运行时数据库，数据模型即「文档内容的结构化描述」。

## 1. 实体

### DocPage（文档页面）

| 字段 | 类型 | 约束 |
|------|------|------|
| `path` | string | 相对 `docs/rust/zh/`，与 `docs/python/zh` 一一对应 |
| `title` | string | frontmatter，必填 |
| `description` | string | frontmatter，一句话，必填 |
| `status` | enum(`已实现`/`部分支持`/`计划中`) | 顶部 `<Note>` 状态块 |
| `compat_level` | enum(`L1`-`L4`, nullable) | 已实现页标注；未实现页 null |
| `mirror_source` | string | 对应 docs/python 相对路径 |
| `example_refs` | list[ExampleRef] | 引用的示例（已实现页 ≥1） |

**状态约束**：`status=计划中` ⇒ 无 Rust 代码块、无 `example_refs`、`compat_level=null`。`status=部分支持` ⇒ 必须列出「已支持 / 尚未实现」边界。

### ExampleRef（示例引用）

| 字段 | 类型 | 约束 |
|------|------|------|
| `crate_name` | string | `examples/<name>` 的 crate 名 |
| `run_command` | string | `cargo run -p <name> -- ...` |
| `credential_required` | bool | 是否需要 `DASHSCOPE_API_KEY` |
| `expected_behavior` | string | 文档声明的预期输出/行为 |

**约束**：`crate_name` 必须存在于 `examples/` 且注册于 workspace members；`run_command` 可实际执行（check 通过）。

### ExampleCrate（示例 crate）

| 字段 | 类型 | 约束 |
|------|------|------|
| `name` | string | 本期 10 个：quickstart/chat/tool/mcp/skill/agent/memory/rag/workspace/sandbox |
| `serves_pages` | list[string] | 服务的文档页路径 |
| `core_demo` | string | 核心演示能力 |
| `deps` | list[string] | workspace 既有依赖（tokio/serde/...），不引入新重依赖 |
| `unsafe` | bool | 恒 false（`#![deny(unsafe_code)]`） |

### MirrorMapEntry（镜像映射条目）

| 字段 | 类型 | 约束 |
|------|------|------|
| `python_page` | string | `docs/python/zh/...` 相对路径 |
| `rust_page` | string | `docs/rust/zh/...` 相对路径 |
| `status` | enum | 同 DocPage.status |
| `compat_level` | enum/null | 同 DocPage |
| `example_crate` | string/`—` | 引用示例或空 |
| `note` | string | 偏差/版本差/例外 |

**约束**：`docs/python/zh` 50 页 ⇒ 50 条；`en/deploy/openapi.json` 为登记例外（不镜像）。

## 2. 关系

- DocPage 1..* ExampleRef 1..1 ExampleCrate（一个示例可服务多个页面）
- MirrorMapEntry 1..1 DocPage（镜像映射是页面的权威登记）
- DocPage.status ↔ 兼容性矩阵 status（三方一致：页面、mirror-map、矩阵）

## 3. 页面清单（50 页，按 docs/python/zh 镜像）

### 顶层（4）
`index.mdx`、`quickstart.mdx`、`release-notes.mdx` + （en 侧 `openapi.json` 例外，无 zh 页）

### building-blocks（36）
- `agent/`：`overview.mdx`、`configure-agent.mdx`、`run-agent.mdx`、`human-in-the-loop.mdx`、`interrupt-agent.mdx`（5）
- `console.mdx`（1）
- `context/`：`overview.mdx`、`compress-context.mdx`、`environment-awareness.mdx`、`offload-context.mdx`（4）
- `long-term-memory.mdx`（1）
- `message-and-event.mdx`（1）
- `middleware.mdx`（1）
- `model/`：`overview.mdx`、`llm.mdx`、`embedding.mdx`、`tts.mdx`（4）
- `permission-system/`：`overview.mdx`、`permission-mode.mdx`、`permission-rule.mdx`、`tool-check.mdx`（4）
- `plan.mdx`（1）
- `rag.mdx`（1）
- `tool/`：`overview.mdx`、`python-tool.mdx`、`mcp.mdx`、`skill.mdx`、`manage-tools.mdx`（5）
- `workspace/`：`overview.mdx`、`manage-resources.mdx`、`mcp-gateway.mdx`、`run-workspace.mdx`（4）
- `session.mdx` — *注意：docs/python 实际无 session.mdx（session 归入 agent/run-agent），不镜像*

### deploy（14）
`agent-service.mdx`、`agent-team.mdx`、`rag.mdx`、`sharing.mdx`、`workspace-manager.mdx` + `channel/`（`overview`/`custom`/`feishu`/`discord`/`routing`）+ `hub/`（`overview`/`mcp-hub`/`skill-hub`）（5+5+3=13... 实为 5 + 5 + 3 = 13，另 agent-service 等 5 = 共 14）

### others（2）
`faq.mdx`、`change-log.mdx`

> 注：页面数量以 `docs/python/zh` 实际文件清单为准（50 页），本清单供 tasks 阶段展开，最终以 mirror-map.md 的 50 条登记为准。

## 4. 示例清单（10 个）

| crate | 服务页面 | 核心演示 |
|-------|----------|----------|
| quickstart | index、quickstart | 最小 Agent：凭据+ChatModel+ToolKit+reply/reply_stream |
| chat | message-and-event、agent/run-agent | 流式对话+EventType 事件分发 |
| tool | tool/overview、tool/python-tool、tool/manage-tools | FunctionTool+ToolKit+内置工具 |
| mcp | tool/mcp | MCP stdio 连接+McpTool 调用 |
| skill | tool/skill | SkillLoader+Skill 工具 |
| agent | agent/*、middleware、permission-system、plan | 编排+权限/人工确认+中断恢复+任务工具 |
| memory | long-term-memory | FileMemory/TurbovecMemory 读写检索 |
| rag | rag | KnowledgeBase+RAGMiddleware(Static/Agentic) |
| workspace | workspace/* | LocalWorkspace 文件操作+内置工具注入 |
| sandbox | workspace/overview(沙箱部分) | SandboxSession 命令执行+路径防护 |

## 5. 校验规则（对齐 quickstart.md 场景）

1. 50 页结构 diff = 无差异（忽略 openapi.json）
2. 每页有状态块（grep）
3. 计划中页无 rust 代码块
4. 示例全过 check+clippy
5. 文档引用示例名与实际一致
6. 无悬空链接；配置项一致
