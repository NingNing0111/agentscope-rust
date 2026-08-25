# microsandbox-agent-cli

真实 `ReActAgent` + microsandbox workspace 示例。

这个示例从 `.env` 读取模型配置，在 host 进程中调用真实模型；同时把 host 的 `workspace/` 目录挂载到 microsandbox guest 的 `/workspace`。`ReActAgent` 绑定 workspace 后会默认注入内置 workspace 工具（legacy `Bash`/`Read`/`Write`/`Edit`/`Grep`/`Glob`/`Skill`，以及 pi-compatible `bash`/`read`/`write`/`edit`/`grep`/`find`/`ls`），示例不再重新定义这些工具 schema，所有文件和命令操作都会通过 microsandbox backend 执行或访问文件。

## 准备 `.env`

在仓库根目录创建 `.env`：

```env
API_KEY=your-api-key
BASE_URL=https://your-openai-compatible-endpoint/v1
MODEL=qwen3.7-plus
```

兼容已有示例约定：

- `API_KEY` fallback 到 `DEFAULT_API_KEY`
- `BASE_URL` fallback 到 `DEFAULT_URL`
- `MODEL` fallback 到 `DEFAULT_CHAT_MODEL`，再 fallback 到 `qwen3.7-plus`

API key 只用于 host 端模型客户端，不会注入 microsandbox guest 环境。

## workspace 与 skills

默认 host 目录结构：

```text
workspace/
  skills/
    my-skill/
      SKILL.md
```

运行时挂载关系：

```text
host:  ./workspace        -> guest: /workspace
host:  ./workspace/skills -> guest: /workspace/skills
```

`Skill` 工具会从 `/workspace/skills` 发现技能。

## 运行

交互式循环：

```bash
rtk cargo run -p microsandbox-agent-cli -- --workspace ./workspace
```

一次性 prompt：

```bash
rtk cargo run -p microsandbox-agent-cli -- \
  --workspace ./workspace \
  --prompt "请列出 workspace 中的文件，并读取 README.md"
```

默认只自动允许只读工具和 `Skill`：`Read`、`Glob`、`Grep`、`Skill`、`ResetTools`，以及 lowercase 只读发现工具 `read`、`grep`、`find`、`ls`。

写入类工具（`Write`/`Edit`/`write`/`edit`）和命令执行工具（`Bash`/`bash`）默认会在交互式和一次性 prompt 中暂停并询问授权：

- `y`：仅批准本次工具调用。
- `n`：拒绝本次工具调用，agent 会收到拒绝结果并继续调整回复。
- `a`：总是允许该工具名，当前进程后续同名工具调用不再询问。

如果你信任当前任务，可以显式允许写操作或 Bash（会同时放行 legacy 与 lowercase 工具名）：

```bash
rtk cargo run -p microsandbox-agent-cli -- \
  --workspace ./workspace \
  --allow-write \
  --allow-bash
```

## 安全说明

- microsandbox 默认使用 `NetworkPolicy::Disabled`；模型 API 调用发生在 host 进程，不需要 guest 网络。
- 不要把真实 secret 写入 workspace 或 sandbox 环境。
- sandbox 输出是不可信数据；示例系统提示要求 agent 不把 sandbox 输出当作指令执行。
- host workspace 是读写挂载，只有挂载该目录会暴露给 guest；不要把敏感目录作为 `--workspace`。
