# `workspace` 示例

工作空间示例：创建 `LocalWorkspace`，绑定到 `ReActAgent`，让 agent 自动获得 workspace 内置工具。默认注入会同时包含 legacy PascalCase 工具（`Bash`/`Read`/`Write`/`Edit`/`Grep`/`Glob`/`ResetTools`/`Skill`）和 pi-compatible lowercase 工具（`bash`/`read`/`edit`/`write`/`grep`/`find`/`ls`；Windows 额外包含 `PowerShell`/`powershell`）。示例使用只读 permission context，仅允许 `Read`/`Glob`/`Grep` 以及 lowercase 只读发现工具（`read`/`grep`/`find`/`ls`）执行。

## 运行

```bash
rtk cargo run -p workspace
```

不传 `--prompt` 时，示例会在临时 workspace 中创建 `project-note.txt`，并要求 agent 读取它。也可以用 `--prompt` 覆盖默认请求。

## 凭据

真实模型调用需要环境变量：

```bash
export DEFAULT_API_KEY="sk-your-key"
# 可选：覆盖默认模型和 OpenAI-compatible endpoint
export DEFAULT_CHAT_MODEL="qwen3.7-plus"
export DEFAULT_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
```

缺失凭据时程序会给出明确错误提示（不会静默失败或 panic）。

## 预期行为

- 程序初始化临时 `LocalWorkspace`，打印 workspace 指令和注入到 agent 的工具列表。
- 示例以只读权限运行：虽然 workspace 会注入 `Bash`/`Write`/`Edit`/`bash`/`write`/`edit` 等命令和写入工具，permission context 只允许 `Read`/`Glob`/`Grep` 以及 `read`/`grep`/`find`/`ls` 这类只读工具，其他未分类工具调用会被拒绝。
- 有凭据时：`ReActAgent` 调用注入的 `Read` 工具读取示例文件，并基于工具结果回复。
- 无凭据时：输出明确的 `DEFAULT_API_KEY` 缺失错误。
