# pi-rust

`pi-rust` 是基于 agentscope-rust crate 构建的 Rust 编码 Agent 示例。`examples/pi-rust/pi-ts` 仅作为功能参考；Rust 实现不导入、不执行、也不依赖其中任何 TypeScript 代码。

## 功能

- DashScope/Qwen 默认模型提供商
- ReActAgent 推理-行动循环
- **ratatui TUI 交互界面**：顶部状态栏、可滚动的消息流（思考内容、助手文本、工具调用/结果实时流式展示）、底部输入框；确认弹窗（y/n/a/d）；`--no-tui` 回退经典 line REPL
- `--prompt` 单轮模式
- `Read`、`Write`、`Edit`、`Bash` 四个编码工具 + `Grep`、`Glob`、`ListDir` 三个搜索浏览工具 + `Memory` 长期记忆写入工具
- 任务闭环：内置 `TaskCreate/TaskList/TaskGet/TaskUpdate` 计划工具，`/tasks` 查看进度，会话记录任务快照
- 确认闭环：破坏性覆盖写与危险 shell 命令经宿主 y/n 批准后自动重试
- `--mode coding` Coding workflow：理解、规划、修改、验证、迭代、汇总
- `--skill-path` 加载 workspace skills，并通过 `Skill` 工具按需读取完整说明
- 工作目录边界校验与可见输出截断
- JSON 会话持久化与 `--resume`
- 长期记忆：`Memory` 工具写入 `workdir/Memory/*.md` 与 `MEMORY.md` 索引，重启后自动注入，跨会话持久
- Ctrl+C 可中断正在运行的 agent 并回到提示符

## 使用

```bash
export API_KEY="sk-your-key"
rtk cargo run -p pi-rust -- --help
rtk cargo run -p pi-rust -- --prompt "请用一句话说明你是什么。"
```

不带 `--prompt` 启动即进入 **TUI 交互界面**（需要真实 TTY；管道/CI 或 `--no-tui` 时自动回退经典 line REPL）：

```bash
rtk cargo run -p pi-rust -- \
  --workdir .pi-rust \
  --cwd . \
  --model qwen-plus \
  --mode coding
```

常用参数：

```bash
rtk cargo run -p pi-rust -- \
  --workdir .pi-rust \
  --cwd . \
  --model qwen-plus \
  --mode coding \
  --show-events \
  --no-tui        # 强制使用 line REPL
```

## TUI 界面

```
┌ pi-rust  dashscope · qwen-plus · mode react · cwd . · skills 0  [running] ┐
│ user  请用 Grep 搜索项目里所有调用 println! 的地方                            │
│ ⋯ 我需要先定位 src/ 下的源文件…                                            │
│ Grep [Grep] {pattern: "println!", path: "src"}                            │
│ → success                                                                  │
│ 找到了 3 处调用 println! 的地方：…                                          │
├──────────────────────────────────────────────────────────────────────────┤
│   running…                                                                │
│ > _                                                                        │
└──────────────────────────────────────────────────────────────────────────┘
```

- 消息流实时流式展示：思考内容（`⋯` 斜体灰）、助手文本、工具调用摘要行与结果（`→ success` / `→ error: …`）
- 按 `Up`/`Down`/`PgUp`/`PgDn` 滚动消息区（新内容到达时自动跟随底部）
- 输入 `/help` 打开帮助覆盖层（`Esc` 或 `q` 关闭）；其他 `/` 命令输出显示到消息流
- 需要宿主确认的操作（危险 shell 命令、覆盖写）弹出确认框：`y` 批准 / `n` 拒绝 / `a` 全部批准 / `d` 全部拒绝 / `Esc` 取消
- `Enter` 发送输入；`Esc` 清空输入框；agent 运行时 `Ctrl+C` 中断，空闲时 `Ctrl+C` 保存并退出

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

### 安装 Anthropic 官方 skills

[`anthropics/skills`](https://github.com/anthropics/skills) 提供 17 个官方 skill（`claude-api`、`pdf`、`docx`、`xlsx`、`pptx`、`frontend-design`、`theme-factory` 等）。安装方式：克隆后把 `skills/` 下的 skill 目录复制到 `.pi-rust/workspace/skills/`。pi-rust 启动时通过 `reconcile` 自动发现新目录，`Skill` 工具也实时扫描该目录——复制即生效，无需重启。

```bash
# 1. 克隆（如未克隆）
git clone https://github.com/anthropics/skills skills

# 2. 复制全部 skill 到运行时 workspace（`skills/skills/` 下每个含 SKILL.md 的子目录是一个 skill）
cp -R skills/skills/. .pi-rust/workspace/skills/
```

等价地，也可在启动时用多个 `--skill-path` 逐个导入（每个参数指向一个含 `SKILL.md` 的目录）：

```bash
rtk cargo run -p pi-rust -- \
  --workdir .pi-rust \
  --skill-path ./skills/skills/claude-api \
  --skill-path ./skills/skills/pdf
```

> 注：anthropics 个别 skill 的 frontmatter 使用 YAML 块标量（`description: |-` 多行描述）。`parse_skill_md`（agent_scope_tool / agent_scope_workspace）已支持块标量解析，`|` 系列保留换行、`>` 系列按折叠语义连接，这类 skill 的多行描述会被完整解析。

## REPL 命令

- `/help`：显示命令、配置摘要和示例 prompt
- `/model`：显示 provider/model，密钥仅脱敏显示
- `/tools`：显示工具和权限行为
- `/skills`：列出已加载 skills
- `/skill NAME`：显示指定 skill 的完整说明
- `/sessions`：列出本地会话
- `/save`：保存当前会话
- `/tasks`：显示 agent 的任务计划/进度/完成状态
- `/approvals`：列出本会话已批准的破坏性操作
- `/context`：显示上下文消息数
- `/events on|off`：切换可读事件输出
- `/json on|off`：切换 JSON 事件输出
- `/exit` 或 `/quit`：保存并退出

### 确认闭环

当 `Write` 覆盖已有文件或 `Bash` 执行危险命令时，工具会先返回 `confirmation_required` 并拒绝执行。随后 REPL 会逐一询问（`y`/`N`）：批准的操作以精确指纹（如 `bash:rm hello.txt`、`write:/abs/path`）记入会话级 approvals，并**自动用同一 prompt 重试**（最多 3 轮），被批准的操作随后正常执行；拒绝的操作不再询问。`/approvals` 可查看已批准项，重启进程后重置。

## 安全模型

- API key 不写入 session JSON。
- 所有文件路径必须位于 `--cwd` 内。
- `Read` 拒绝目录、二进制或非 UTF-8 文件。
- `Write` 默认不覆盖已有文件，除非工具调用明确设置 `overwrite=true`；覆盖已有文件会返回 `confirmation_required`，当前示例不把模型自填字段视为真实用户确认。
- `Edit` 使用精确字符串替换；默认要求匹配项唯一。
- `Bash` 在 `--cwd` 中执行，并对 `rm`、`git reset`、安装命令、写重定向、网络脚本等风险命令返回 `confirmation_required`。
- `Grep`/`Glob`/`ListDir` 只读，跳过隐藏条目（`.` 前缀）与符号链接，并受结果数上限约束。
- `Memory` 仅写入 `--workdir/Memory/`（记忆文件名会归一化为安全 ASCII 组件），不接触工作目录内其他文件；写入立即可见，`--no-memory` 时工具返回 `memory_disabled`。

## 会话布局

默认工作目录为 `.pi-rust/`：

```text
.pi-rust/
├── sessions/   # JSON 会话记录
└── Memory/     # 长期记忆（Memory 工具写入 + MEMORY.md 索引）
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
