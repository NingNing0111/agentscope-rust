# AgentScope Rust

Rust 重构版 AgentScope 示例仓库。核心能力位于 `crates/agent_scope_*`，当前可直接体验的完整交互式编码 Agent 位于 `examples/pi-rust`。

## 快速体验 pi-rust 编码 Agent

创建仓库根目录 `.env` 或设置环境变量：

```bash
echo 'API_KEY=sk-your-real-dashscope-key' > .env
```

查看命令行参数：

```bash
cargo run -p pi-rust -- --help
```

一次性发送 prompt 后退出：

```bash
cargo run -p pi-rust -- --prompt "请用一句话说明你是什么。"
```

不带 `--prompt` 启动交互式 TUI（真实 TTY 中启用；管道/CI 或 `--no-tui` 时回退 line REPL）：

```bash
cargo run -p pi-rust -- \
  --workdir .pi-rust \
  --cwd . \
  --model qwen-plus \
  --mode coding
```

显示可读事件并强制使用 line REPL：

```bash
cargo run -p pi-rust -- \
  --mode coding \
  --show-events \
  --no-tui
```

`pi-rust` 会从 `.env`/`API_KEY` 或 `--api-key` 读取 DashScope 密钥，并对输出中的密钥脱敏。它展示真实 DashScope/Qwen ReActAgent、流式事件、ratatui TUI、确认闭环、任务规划工具、编码工具（Read/Write/Edit/Bash）、搜索浏览工具（Grep/Glob/ListDir）、长期记忆 Memory 工具、workspace skills、会话持久化与 `--resume`。完整说明见 `examples/pi-rust/README.md`。

> 开发者提示：本仓库开发时可按 `CLAUDE.md` 使用 `rtk cargo ...` 获取更紧凑的构建/测试输出；普通用户直接使用上面的裸 `cargo` 命令即可。

## 相关资源

- TurboVec: https://github.com/RyanCodrai/turbovec
- microsandbox: https://github.com/superradcompany/microsandbox
