# pi-rust

`pi-rust` 是基于 agentscope-rust crate 构建的 Rust 编码 Agent 示例。`examples/pi-rust/pi-ts` 仅作为功能参考；Rust 实现不导入、不执行、也不依赖其中任何 TypeScript 代码。

## 功能

- DashScope/Qwen 默认模型提供商
- ReActAgent 推理-行动循环
- 交互式 REPL 与 `--prompt` 单轮模式
- `Read`、`Write`、`Edit`、`Bash` 四个编码工具
- `--mode coding` Coding workflow：理解、规划、修改、验证、迭代、汇总
- `--skill-path` 加载 workspace skills，并通过 `Skill` 工具按需读取完整说明
- 工作目录边界校验与可见输出截断
- JSON 会话持久化与 `--resume`
- 可选 MemoryMiddleware

## 使用

```bash
export API_KEY="sk-your-key"
rtk cargo run -p pi-rust -- --help
rtk cargo run -p pi-rust -- --prompt "请用一句话说明你是什么。"
```

常用参数：

```bash
rtk cargo run -p pi-rust -- \
  --workdir .pi-rust \
  --cwd . \
  --model qwen-plus \
  --mode coding \
  --show-events
```

## Skills

可以通过 `--skill-path` 导入包含 `SKILL.md` 的技能目录。`pi-rust` 会把有效 skills 复制到 `.pi-rust/workspace/skills/`，将 `<agent-skills>` 注入 system prompt，并在有 skills 时注册只读 `Skill` 工具。

```bash
rtk cargo run -p pi-rust -- \
  --mode coding \
  --skill-path ./skills/rust-coding
```

`SKILL.md` 格式：

```markdown
---
name: rust-coding
description: Rust coding workflow guidance
---

# Rust Coding

具体技能说明。
```

没有加载 skills 时，`Skill` 工具不会暴露，模型也不应声称 skills 可用。

## REPL 命令

- `/help`：显示命令、配置摘要和示例 prompt
- `/model`：显示 provider/model，密钥仅脱敏显示
- `/tools`：显示工具和权限行为
- `/skills`：列出已加载 skills
- `/skill NAME`：显示指定 skill 的完整说明
- `/sessions`：列出本地会话
- `/save`：保存当前会话
- `/events on|off`：切换可读事件输出
- `/json on|off`：切换 JSON 事件输出
- `/exit` 或 `/quit`：保存并退出

## 安全模型

- API key 不写入 session JSON。
- 所有文件路径必须位于 `--cwd` 内。
- `Read` 拒绝目录、二进制或非 UTF-8 文件。
- `Write` 默认不覆盖已有文件，除非工具调用明确设置 `overwrite=true`；覆盖已有文件会返回 `confirmation_required`，当前示例不把模型自填字段视为真实用户确认。
- `Edit` 使用精确字符串替换；默认要求匹配项唯一。
- `Bash` 在 `--cwd` 中执行，并对 `rm`、`git reset`、安装命令、写重定向、网络脚本等风险命令返回 `confirmation_required`。

## 会话布局

默认工作目录为 `.pi-rust/`：

```text
.pi-rust/
├── sessions/   # JSON 会话记录
└── Memory/     # MemoryMiddleware 文件存储
```

恢复最近会话：

```bash
rtk cargo run -p pi-rust -- --resume
```

恢复指定会话：

```bash
rtk cargo run -p pi-rust -- --resume <SESSION_ID>
```

## 验证

```bash
rtk cargo fmt --check
rtk cargo clippy -p pi-rust --all-targets -- -D warnings
rtk cargo test -p pi-rust
rtk cargo check -p pi-rust
rtk cargo run -p pi-rust -- --help
```

真实 DashScope 端到端验证需要有效 `API_KEY`，请参考 `specs/023-pi-coding-agent/quickstart.md`。
