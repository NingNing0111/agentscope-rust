# pi-rust TUI 视觉面板风升级设计

- **日期**: 2026-08-07
- **范围**: `examples/pi-rust/src/tui.rs`(仅渲染层)
- **目标**: 在不改动事件流、`agent_task`、`render.rs` 与 line REPL 的前提下,将 ratatui TUI 的展示升级为"现代面板风"——带边框分区、角色化前缀、工具调用与结果视觉绑定、Meta 噪音降噪、长文本自动换行。

## 背景与动机

`examples/pi-rust` 的交互模式默认进入 ratatui TUI(双任务 channel 架构,见 `pi-rust-tui-integration` 记忆)。当前展示的问题:

1. **缺少分区与边框** —— 所有内容挤在一起,难以一眼区分用户/助手/工具活动;头部信息过长时易溢出换行。
2. **层次不够分明** —— 流式正文为纯默认色、思考块为灰斜体,重点不突出;工具调用与结果行没有绑定/缩进分组。
3. **整体平淡** —— 事件行(`· MODEL_CALL_START` 等)是噪音,状态行单调,无视觉亮点。
4. **长文本溢出** —— 文本与工具摘要不自动换行,长内容横向溢出。

## 约束

- 事件流、`UiMsg` 消息类型、channel 容量、`agent_task`、`run_turn_for_tui` 的合并/背压逻辑**全部保持不变**。
- `render.rs::render_event` 及其对 line REPL 的输出语义**保持不变**。
- 现有 18 个单元测试尽量复用,行为契约尽量不破坏。
- 不引入新依赖(ratatui 0.30 + crossterm 0.29 已满足)。

## 架构决策

### 1. Theme 集中配色

新增 `Theme` 结构,集中定义语义色,替换散落的 `Color::Cyan/Green/Red/...`:

```rust
struct Theme {
    accent:   Color, // Cyan     —— logo、工具名、输入提示符 `>`、主面板边框
    success:  Color, // Green    —— 工具 `→ success`、`you` 前缀、idle 状态点
    error:    Color, // Red      —— 工具 `→ error/denied`
    warn:     Color, // Yellow   —— busy 状态、中断提示
    muted:    Color, // DarkGray —— Meta/系统行、状态栏快捷键提示
    thinking: Color, // Magenta  —— 思考块 `⋯`(配合斜体)
    border:   Color, // DarkGray —— 分隔线、辅助边框
}
```

`App` 持有一个 `theme: Theme` 实例,所有渲染方法通过它取色。测试可通过 `theme_colors_are_consistent` 校验色值语义。

### 2. 布局 Panel 化

`Layout::vertical` 维持四区(header / message / status / input),但 Header 与消息区升级为带边框面板:

```
┌ pi-rust · qwen-plus · react ─ cwd ─ skills 5 ───── running ┐
│  消息区内容                                                 │
├─ message ─────────────────────────────────────────────────┤
│  …滚动内容…                                                 │
├─ status ──────────────────────────────────────────────────┤
│  [ok] Ready                       /help · ctrl+c quit      │
└────────────────────────────────────────────────────────────┘
```

- **Header**: 左端 `pi-rust`(黑底青字徽标)+ 元信息;右端 busy/idle 状态点(`● running` 黄 / `● idle` 绿),右上角收窄边框(`┐`)。
- **消息区**: 带 `· message ·` 标题边框;主边框 accent 色,内容区启用 `Wrap`。
- **状态栏**: `[ok]/[busy]/[err]` 前缀 + 左侧 status;右侧常驻暗色快捷键提示(`/help · ctrl+c quit`)。
- **输入栏**: `> `(accent)+ 输入;busy 时 `>` 变黄。
- **窄终端兜底**: `area.width < 60` 时面板边框退化为无边框(仅保留内容),避免小窗口信息挤压。

### 3. 消息区角色化渲染 + 工具块绑定

`UiItem::ToolCall` 改为携带可变的 `result: Option<String>` 与 `tool_call_id: String`。`ToolResultEnd` 到达时**按 `tool_call_id` 精确匹配**并**原地更新**对应 ToolCall 项(而非"最近一个未闭合",避免多工具并行时结果乱序),实现调用与结果视觉绑定:

```
you  请优化项目结构

⋯ 让我先看看目录…

Bash  $ ls -la
      → success
Bash  $ grep -r "todo" src
      → error: (截断摘要)
```

- `you` 前缀: 绿粗。
- 助手正文: 默认色,`Wrap` 自动换行。
- 思考块: `⋯ ` 品红斜体。
- 工具块: 调用行 + 缩进对齐的结果行,结果按成功/失败着色。

### 4. Meta 事件降噪

删除 `ModelCallStart / ModelCallEnd / ReplyStart / ReplyEnd` 四个事件各自 push 一行的行为——它们是纯噪音。工具活动由工具块清晰呈现,模型是否在工作由 header 的 `● running` 状态点体现。保留 `UserInterrupt` 等有意义的系统行。

## 数据流 / 错误处理 / 不变量

- **数据流不变**: 事件 → `consume_event` → `items` → `render` → `terminal.draw`。工具块原地更新只改 `consume_event` 对 `items` 的写入,不新增 channel/消息类型。
- **错误处理不变**: draw 错误继续走 `PiError::io`。
- **不变量**: `ToolResultEnd` 一定出现在对应 `ToolCallEnd` 之后(事件流保证),且二者 `tool_call_id` 一致。若因异常找不到对应 `tool_call_id` 的 ToolCall,退化为追加独立结果行(不 panic)。

## 测试调整

- 更新 `tool_call_and_result_produce_summary_items`: 断言 ToolCall 项原地持有 result(`result: Some("→ success")`),不再产生独立 ToolResult item。
- 新增:
  - `tool_result_updates_latest_open_tool_call`(结果绑定正确性,含 thinking 穿插场景)
  - `meta_events_no_longer_produce_items`(降噪验证)
  - `theme_colors_are_consistent`(Theme 色值 sanity)
  - `narrow_terminal_falls_back_to_no_border`(窄终端兜底逻辑)
- 其余 17 个现有测试不受影响。

## 验证

- `rtk cargo clippy -p pi-rust --all-targets -- -D warnings` 0 错误
- `rtk cargo test -p pi-rust` 全部通过
- `cargo fmt --check` clean
- 手动运行 TUI 目检: 面板边框、角色前缀、工具块绑定、状态点、长文本换行。
