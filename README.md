# AgentScope Rust

Rust 重构版 AgentScope 示例仓库。核心能力位于 `crates/agent_scope_*`，仓库根 package 主要用于运行 examples。

## 快速体验完整 Agent Demo

创建仓库根目录 `.env`：

```bash
echo 'API_KEY=sk-your-real-dashscope-key' > .env
```

启动调用真实 DashScope API 的交互式 Agent：

```bash
rtk cargo run --example agent_demo
```

显示模型、工具、权限和回复生命周期事件：

```bash
rtk cargo run --example agent_demo -- --model qwen-plus --show-events
```

一次性发送 prompt 后退出：

```bash
rtk cargo run --example agent_demo -- --prompt "请用 calculator 计算 23 * (17 + 5)"
```

`agent_demo` 会从 `.env`/`API_KEY` 或 `--api-key` 读取密钥，并在终端输出和 JSON event 输出中对密钥脱敏。它展示真实 DashScope ReActAgent、流式事件、FunctionTool、`Skill` 技能说明查询、显式 memory 写入/查询、MemoryMiddleware、LocalWorkspace、Static RAG 和权限拒绝。完整说明见 `examples/agent-demo/README.md`。

## 相关资源

- TurboVec: https://github.com/RyanCodrai/turbovec
- microsandbox: https://github.com/superradcompany/microsandbox
