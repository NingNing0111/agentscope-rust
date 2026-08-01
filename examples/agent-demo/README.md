# AgentScope Rust Interactive Agent

`agent_demo` 是一个真实可用的交互式 Agent 示例。它使用真实 DashScope chat API、可选的 DashScope embedding + Turbovec RAG、真实 `FileMemory`、真实 `LocalWorkspace`，并且只从 workspace 中读取实际存在的 skills。

> 这个示例会调用真实 DashScope API；启用 RAG 时还会调用真实 DashScope embedding API，可能产生模型服务费用。不要在终端、文档或 issue 中粘贴真实 API Key。

## 1. 准备 API Key

在仓库根目录创建 `.env`：

```bash
echo 'API_KEY=sk-your-real-key' > .env
```

示例入口会通过 `dotenv` 加载 `.env`，然后把 key 显式传给 DashScope chat 和 embedding provider。也可以直接使用环境变量或 CLI 参数：

```bash
API_KEY=sk-your-real-key cargo run --example agent_demo
cargo run --example agent_demo -- --api-key sk-your-real-key
```

## 2. 启动交互式 Agent

默认启动 REPL：

```bash
cargo run --example agent_demo
```

指定模型并显示事件边界：

```bash
cargo run --example agent_demo -- --model qwen-plus --show-events
```

打印脱敏后的 `AgentEvent` JSON：

```bash
cargo run --example agent_demo -- --show-json-events
```

发送一次性 prompt 后退出：

```bash
cargo run --example agent_demo -- --prompt "请用 calculator 计算 23 * (17 + 5)"
```

运行数据默认写入 `.agent-demo/`。可以显式指定目录或关闭某项能力：

```bash
cargo run --example agent_demo -- --workdir .agent-demo-local
cargo run --example agent_demo -- --no-memory --prompt "memory 功能是否启用？"
cargo run --example agent_demo -- --no-workspace --prompt "workspace 功能是否启用？"
cargo run --example agent_demo -- --no-rag --prompt "RAG 功能是否启用？"
```

说明：workspace 初始化会在 workdir 下创建 `workspace/`、`.mcp`、`skills/`、`sessions/`、`data/` 等运行文件；memory 会在 workdir 下创建 `Memory/` 和 `MEMORY.md`。示例不会预置 memory、skill 或 RAG 文档。每轮对话会追加落盘到 `workspace/sessions/<session_id>/context.jsonl`；`data/` 只在消息或工具结果包含 base64 `DataBlock` 并触发 offload 时写入真实数据文件。

## 3. RAG：真实文档 + DashScope embedding + Turbovec

默认不会内置任何知识库。未提供 `--rag-doc` 或 `--rag-dir` 时，RAG 不启用。

加载单个真实文档：

```bash
cargo run --example agent_demo -- \
  --rag-doc README.md \
  --show-events \
  --prompt "请基于已加载文档总结这个项目"
```

加载多个文档：

```bash
cargo run --example agent_demo -- \
  --rag-doc README.md \
  --rag-doc docs/zh/getting-started.md \
  --prompt "文档里如何介绍快速开始？"
```

加载目录：

```bash
cargo run --example agent_demo -- \
  --rag-dir docs \
  --rag-recursive \
  --prompt "文档里有哪些入门步骤？"
```

支持的文档格式：`.txt`、`.md`、`.markdown`、`.text`。

RAG 相关参数：

```text
--embedding-model <MODEL>   DashScope embedding model，默认 text-embedding-v3
--rag-top-k <N>             每次检索注入的 chunk 数，默认 3
--rag-threshold <FLOAT>     可选分数阈值
--rag-chunk-size <N>        文档分块大小，默认 500
--rag-overlap <N>           相邻分块重叠，默认 80
--rag-collection <NAME>     Turbovec collection 名称，默认 agent_demo_docs
```

RAG 初始化流程是真实执行：读取本地文档 → `TextParser` 解析 → `ApproxTokenChunker` 分块 → `DashScopeEmbeddingModel` 生成 embedding → `TurbovecVectorStore` 建索引 → `RAGMiddleware` 在回复前检索并注入相关上下文。

## 4. Workspace skills

skills 只来自 `LocalWorkspace`，没有就是没有。

- 若 workspace 没有发现任何 `SKILL.md`，示例不会注册 `Skill` 工具，也不会注入 `<agent-skills>` prompt。
- 可以用 `--skill-path` 导入真实 skill 目录：

```bash
cargo run --example agent_demo -- \
  --skill-path ./path/to/my-skill \
  --prompt "请查看可用技能并按技能说明回答"
```

skill 目录应包含 `SKILL.md`，格式示例：

```markdown
---
name: my-skill
description: Instructions for a real workflow
---

# My Skill

Use these instructions when the user asks for ...
```

也可以把真实 skill 放到 `<workdir>/workspace/skills/` 下，然后重新运行示例。

## 5. REPL 命令

```text
/help          Show help
/model         Show current model
/tools         Show configured tool categories
/events on     Enable lifecycle event rendering
/events off    Disable lifecycle event rendering
/json on       Enable redacted AgentEvent JSON output
/json off      Disable JSON event output
/exit, /quit   Quit
```

## 6. 工具

工具按实际启用能力注册。`--no-tools` 会禁用全部工具。

| Tool | 条件 | 用途 |
|------|------|------|
| `calculator` | tools enabled | 安全计算基础算术表达式，支持 `+ - * /`、括号和一元负号 |
| `safe_time` | tools enabled | 返回真实系统时间 / Unix 时间 |
| `workspace_info` | workspace enabled | 展示当前 `LocalWorkspace` id、目录和运行状态 |
| `workspace_list_tools` | workspace enabled | 列出 workspace 内置工具 inventory |
| `Bash` | workspace enabled | 在 workspace 目录内执行受限只读 shell 诊断命令，返回 cwd、exit code、stdout、stderr |
| `workspace_write_file` | workspace enabled | 在终端确认后，将 UTF-8 文本文件写入 workspace 内，拒绝绝对路径和 path traversal |
| `Skill` | workspace skills discovered | 按精确技能名读取 workspace 中真实 skill 的说明 |
| `memory_write` | memory enabled | 保存或更新非敏感的持久记忆 |
| `memory_search` | memory enabled | 按关键词和可选类型过滤查询持久记忆 |
| `memory_read` | memory enabled | 按精确名称读取一条持久记忆 |
| `memory_list` | memory enabled | 列出持久记忆条目的名称、类型和描述 |

RAG 通过 middleware 注入上下文，不注册单独的 demo 查询工具。

`Bash` 工具固定在 `<workdir>/workspace/` 内运行。`pwd`、`ls`、`find`、`grep`、`cat`、`wc`、`date`、版本检查等低风险诊断命令会直接执行；写入/删除/移动/改权限、安装包、外联网络、git 写操作、写重定向、访问 `/tmp` 等风险命令会先在终端请求确认，用户输入 `y` / `yes` 后才执行；空命令、明显长运行命令和路径逃逸（例如 `..`、`/etc`、`/Users`）仍会硬拒绝。需要写 UTF-8 文件时优先使用 `workspace_write_file`。

## 7. 确认式写入示例

当用户要求创建或写入文件时，模型应调用 `workspace_write_file`。工具会把目标限制在 `<workdir>/workspace/` 内，并在终端确认后才写入：

```text
you> 请创建 hello.txt，写入 Hello World!
assistant>
[tool:start] workspace_write_file (...)
[permission:confirm] workspace_write_file wants to write 12 byte(s) to "hello.txt" inside .agent-demo/workspace
Allow this write? Type 'y' or 'yes' to continue: y
[tool:result] workspace_write_file wrote 12 byte(s) to "hello.txt" inside the workspace.
```

安全边界：

- 只接受相对路径，例如 `hello.txt` 或 `notes/hello.txt`；
- 拒绝绝对路径，例如 `/tmp/hello.txt`；
- 拒绝 `..` path traversal，例如 `../hello.txt`；
- 用户输入非 `y` / `yes` 时不会写入文件。

## 8. 验证

```bash
cargo fmt --check
cargo check --example agent_demo
cargo build --example agent_demo
```

无 RAG 文档时：

```bash
cargo run --example agent_demo -- --prompt "请一句话介绍你能做什么"
cargo run --example agent_demo -- --no-rag --prompt "请调用 Bash 执行 pwd，并告诉我返回了什么"
```

运行后可在 `workspace/sessions/<session_id>/context.jsonl` 查看本轮 user/assistant 消息，例如：

```bash
find .agent-demo/workspace/sessions -name context.jsonl -print
```

真实 RAG 单文档：

```bash
cargo run --example agent_demo -- \
  --rag-doc README.md \
  --show-events \
  --prompt "请基于已加载文档总结这个项目"
```

真实 workspace skill：

```bash
cargo run --example agent_demo -- \
  --skill-path ./path/to/my-skill \
  --show-events \
  --prompt "请读取可用技能并总结技能要求"
```

如果没有配置 `API_KEY`，示例会在调用 provider 之前失败，并提示如何创建 `.env`。
