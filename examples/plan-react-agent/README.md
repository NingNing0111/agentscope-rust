# `plan-react-agent` 示例

计划模式（Plan mode）示例：`ReActAgent` 内置任务规划工具（`TaskCreate` / `TaskList` / `TaskGet` / `TaskUpdate`）的完整生命周期。

## 运行

```bash
cargo run -p plan-react-agent -- --help
```

## 演示内容

1. **工具注册检查**（无需模型）——打印四个内置任务工具的注册状态；
2. **模型驱动的规划循环**——流式消费 `reply_stream`，观察模型通过工具调用走完 `TaskCreate` → `TaskList` → `TaskUpdate(in_progress)` → `TaskUpdate(completed)`；
3. **状态持久化**——回复结束后从 `agent.try_state().tasks_context.tasks` 读取并打印最终任务清单。

## 凭据

真实模型调用需要环境变量：

```bash
export DASHSCOPE_API_KEY="sk-your-key"
```

缺失凭据时程序会给出明确错误提示（不会静默失败或 panic）。
