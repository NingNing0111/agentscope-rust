# pi-rust TUI 视觉面板风升级 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `examples/pi-rust` 的 ratatui TUI 展示升级为"现代面板风"——带边框分区、角色化前缀、工具调用与结果视觉绑定、Meta 噪音降噪、长文本自动换行。

**Architecture:** 所有改动集中在 `examples/pi-rust/src/tui.rs` 渲染层。事件流、`UiMsg` 类型、`agent_task`、`render.rs`、line REPL 均不动。核心是:新增 `Theme` 集中配色;`UiItem::ToolCall` 携带 `tool_call_id` + 可变 `result` 实现结果原地绑定;`items_to_lines` 角色化渲染;布局 Panel 化。

**Tech Stack:** Rust 2024 · ratatui 0.30 · crossterm 0.29

## Global Constraints

- 仅修改 `examples/pi-rust/src/tui.rs`(含其 `#[cfg(test)]` 模块)。
- 不新增依赖。
- 事件流、`UiMsg` 枚举、channel 容量(256)、`agent_task`、`run_turn_for_tui` 的 delta 合并/背压逻辑保持不变。
- `render.rs::render_event` 及 line REPL 输出语义保持不变。
- 现有 18 个单元测试中,除 `tool_call_and_result_produce_summary_items` 外全部保持通过。
- 不变量:`ToolResultEnd` 的 `tool_call_id` 必与某个 `ToolCallEnd` 匹配;找不到时退化为追加独立结果行,不 panic。

---

### Task 1: Theme 集中配色 + UiItem 工具块携带 result

**Files:**
- Modify: `examples/pi-rust/src/tui.rs`(UiItem 枚举 ~65 行、App::new ~145 行、consume_event ~355 行、items_to_lines ~582 行、tests 模块)

**Interfaces:**
- Produces:
  - `struct Theme { accent, success, error, warn, muted, thinking, border: Color }` 带 `impl Default`
  - `App.theme: Theme` 字段
  - `UiItem::ToolCall { name: String, summary: String, tool_call_id: String, result: Option<String> }`
  - `App::consume_event` 的 `ToolCallEnd` 分支 push 带 `tool_call_id` 的 ToolCall;`ToolResultEnd` 分支按 `tool_call_id` 原地更新 `result`

- [ ] **Step 1: 更新 `tool_call_and_result_produce_summary_items` 测试为失败态**

将 `tui.rs` 测试模块中该测试改为断言新行为:ToolCall 项原地持有 result,不再产生独立 ToolResult item:

```rust
#[test]
fn tool_call_and_result_produce_summary_items() {
    let mut app = App::new(&config(), 0);
    app.consume_event(AgentEvent::ToolCallStart(
        agent_scope_event::ToolCallStartEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
            tool_call_id: "tc-1".into(),
            tool_call_name: "Bash".into(),
        },
    ));
    app.consume_event(AgentEvent::ToolCallEnd(
        agent_scope_event::ToolCallEndEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
            tool_call_id: "tc-1".into(),
            input: Some(r#"{"command":"ls"}"#.into()),
        },
    ));
    app.consume_event(AgentEvent::ToolResultEnd(
        agent_scope_event::ToolResultEndEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
            tool_call_id: "tc-1".into(),
            state: agent_scope_message::ToolResultState::Success,
            metadata: Default::default(),
            output: Some("exit_code: 0".into()),
        },
    ));
    assert!(matches!(
        app.items[0],
        UiItem::ToolCall { ref name, ref summary, ref result, .. }
            if name == "Bash" && summary.contains("ls") && result.as_deref() == Some("→ success")
    ));
    assert_eq!(app.items.len(), 1, "ToolResult must fold into the ToolCall item");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `rtk cargo test -p pi-rust tool_call_and_result_produce_summary_items`
Expected: FAIL(编译错误:ToolCall 变体字段不匹配)

- [ ] **Step 3: 实现 Theme 结构**

在 `App` 定义前新增:

```rust
/// 集中定义的语义配色,替换散落在渲染方法里的字面色值。
#[derive(Debug, Clone, Copy)]
struct Theme {
    accent: Color,   // 主色:logo、工具名、输入提示符 `>`、主面板边框
    success: Color,  // 成功:工具 `→ success`、`you` 前缀、idle 状态点
    error: Color,    // 失败:工具 `→ error/denied`
    warn: Color,     // 忙碌:busy 状态、中断提示
    muted: Color,    // 次要:Meta/系统行、状态栏快捷键提示
    thinking: Color, // 思考:思考块 `⋯`
    border: Color,   // 边框:分隔线、辅助边框
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            success: Color::Green,
            error: Color::Red,
            warn: Color::Yellow,
            muted: Color::DarkGray,
            thinking: Color::Magenta,
            border: Color::DarkGray,
        }
    }
}
```

- [ ] **Step 4: 改造 UiItem::ToolCall 并给 App 加 theme 字段**

```rust
pub enum UiItem {
    System(String),
    UserMsg(String),
    StreamText(String),
    StreamThinking(String),
    ToolCall {
        name: String,
        summary: String,
        tool_call_id: String,
        /// 结果行(`→ success` / `→ error: …`),ToolResultEnd 到达时原地填充。
        result: Option<String>,
    },
    ToolResult(String),
    Meta(String),
}
```

`App` 结构体新增 `theme: Theme,` 字段;`App::new` 中 `theme: Theme::default(),`。

- [ ] **Step 5: 改造 consume_event 的 ToolCallEnd / ToolResultEnd 分支**

```rust
AgentEvent::ToolCallEnd(end) => {
    let name = self
        .tool_call_names
        .get(&end.tool_call_id)
        .cloned()
        .unwrap_or_else(|| "?".to_string());
    let summary = tool_call_summary(&name, end.input.as_deref());
    self.items.push(UiItem::ToolCall {
        name,
        summary,
        tool_call_id: end.tool_call_id.clone(),
        result: None,
    });
    self.follow_bottom = true;
}
AgentEvent::ToolResultEnd(end) => {
    let line = tool_result_line(&self.tool_outputs, end);
    // 按 tool_call_id 精确匹配,原地绑定结果到对应调用项。
    let tool_call_id = end.tool_call_id.clone();
    let matched = self.items.iter_mut().rev().find_map(|item| match item {
        UiItem::ToolCall {
            tool_call_id: id,
            result,
            ..
        } if *id == tool_call_id && result.is_none() => {
            *result = Some(line.clone());
            Some(())
        }
        _ => None,
    });
    if matched.is_none() {
        // 不变量退化路径:找不到对应调用项时追加独立结果行。
        self.items.push(UiItem::ToolResult(line));
    }
    self.follow_bottom = true;
}
```

- [ ] **Step 6: 更新 items_to_lines 的 ToolCall / ToolResult 分支**

```rust
UiItem::ToolCall {
    name,
    summary,
    result,
    ..
} => {
    lines.push(Line::from(vec![
        Span::styled(
            format!("{name}  "),
            Style::default()
                .fg(self.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(summary.clone(), Style::default().fg(self.theme.accent)),
    ]));
    if let Some(result) = result {
        let color = if result.starts_with("→ success") {
            self.theme.success
        } else {
            self.theme.error
        };
        // 缩进对齐工具名后,与调用行视觉绑定。
        lines.push(Line::from(Span::styled(
            format!("      {result}"),
            Style::default().fg(color),
        )));
    }
}
UiItem::ToolResult(text) => {
    let color = if text.starts_with("→ success") {
        self.theme.success
    } else {
        self.theme.error
    };
    lines.push(Line::from(Span::styled(
        text.clone(),
        Style::default().fg(color),
    )));
}
```

- [ ] **Step 7: 新增 tool_result_updates_latest_open_tool_call 测试**

```rust
#[test]
fn tool_result_updates_latest_open_tool_call() {
    let mut app = App::new(&config(), 0);
    // 两个并行工具调用(事件流允许交错),结果必须按 id 精确绑定。
    for (id, name, input) in [
        ("tc-a", "Bash", r#"{"command":"ls"}"#),
        ("tc-b", "Read", r#"{"path":"a.txt"}"#),
    ] {
        app.consume_event(AgentEvent::ToolCallStart(
            agent_scope_event::ToolCallStartEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                tool_call_id: id.into(),
                tool_call_name: name.into(),
            },
        ));
        app.consume_event(AgentEvent::ToolCallEnd(
            agent_scope_event::ToolCallEndEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                tool_call_id: id.into(),
                input: Some(input.into()),
            },
        ));
    }
    // tc-b 的结果先到。
    app.consume_event(AgentEvent::ToolResultEnd(
        agent_scope_event::ToolResultEndEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
            tool_call_id: "tc-b".into(),
            state: agent_scope_message::ToolResultState::Error,
            metadata: Default::default(),
            output: Some("boom".into()),
        },
    ));
    // tc-a 的结果后到。
    app.consume_event(AgentEvent::ToolResultEnd(
        agent_scope_event::ToolResultEndEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
            tool_call_id: "tc-a".into(),
            state: agent_scope_message::ToolResultState::Success,
            metadata: Default::default(),
            output: Some("exit_code: 0".into()),
        },
    ));
    assert_eq!(app.items.len(), 2, "both results must fold into their own items");
    let a = &app.items[0];
    let b = &app.items[1];
    assert!(matches!(
        a,
        UiItem::ToolCall { ref result, .. } if result.as_deref() == Some("→ success")
    ));
    assert!(matches!(
        b,
        UiItem::ToolCall { ref result, .. } if result.as_deref().is_some_and(|r| r.starts_with("→ error"))
    ));
}
```

- [ ] **Step 8: 运行全部测试并提交**

Run: `rtk cargo test -p pi-rust`
Expected: 全部通过
Run: `rtk cargo clippy -p pi-rust --all-targets -- -D warnings`
Expected: 0 错误
Run: `cargo fmt --check`
Expected: clean

```bash
git add examples/pi-rust/src/tui.rs
git commit -m "feat(pi-rust): TUI 工具调用与结果按 id 绑定,引入 Theme 配色"
```

---

### Task 2: 布局 Panel 化(带边框面板 + 状态栏前缀 + 窄终端兜底)

**Files:**
- Modify: `examples/pi-rust/src/tui.rs`(render ~480 行、render_header ~502 行、render_message_area ~523 行、render_status ~539 行、render_input ~549 行、tests 模块)

**Interfaces:**
- Consumes: Task 1 的 `App.theme: Theme`
- Produces: `App::render` 的 4 区 Panel 布局;窄终端(<60 列)退化为无边框

- [ ] **Step 1: 新增窄终端兜底测试**

```rust
#[test]
fn narrow_terminal_falls_back_to_no_border() {
    // <60 列时不绘制面板边框,布局仍为 4 区、不 panic。
    let mut app = App::new(&config(), 0);
    let wide = ratatui::layout::Rect::new(0, 0, 100, 10);
    let narrow = ratatui::layout::Rect::new(0, 0, 40, 10);
    assert!(App::bordered(wide));
    assert!(!App::bordered(narrow));

    let [header, main, status, input] = app.layout_areas(wide);
    assert_eq!(header.width, wide.width);
    assert_eq!(main.width, wide.width);
    assert_eq!(status.width, wide.width);
    assert_eq!(input.width, wide.width);
    let _ = (header, main, status, input);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `rtk cargo test -p pi-rust narrow_terminal_falls_back_to_no_border`
Expected: FAIL(编译错误:`bordered` / `layout_areas` 方法不存在)

- [ ] **Step 3: 新增 bordered 判定与 layout_areas 方法**

在 `impl App` 中新增:

```rust
/// 宽终端(≥60 列)绘制面板边框,窄终端跳过以节省空间。
fn bordered(area: Rect) -> bool {
    area.width >= 60
}

/// 将屏幕分为 header/message/status/input 四区。
fn layout_areas(area: Rect) -> [Rect; 4] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area)
}
```

- [ ] **Step 4: 重写 render 调用 bordered + layout_areas + 面板化**

```rust
fn render(&mut self, frame: &mut Frame) {
    let area = frame.area();
    let [header, main, status, input] = App::layout_areas(area);
    let bordered = App::bordered(area);

    if bordered {
        self.render_header_panel(frame, header);
        self.render_message_panel(frame, main);
        self.render_status_panel(frame, status);
    } else {
        self.render_header(frame, header);
        self.render_message_area(frame, main);
        self.render_status(frame, status);
    }
    self.render_input(frame, input);

    match self.mode {
        Mode::Confirm => self.render_overlay(frame, "confirm", self.confirm_lines()),
        Mode::Help => self.render_overlay(frame, "help", self.help_lines()),
        Mode::Input => {}
    }
}
```

- [ ] **Step 5: 新增三个面板渲染方法(保留原无边框方法供兜底复用)**

```rust
fn render_header_panel(&self, frame: &mut Frame, area: Rect) {
    let (left, right) = self.header_spans();
    let block = Block::default().borders(Borders::TOP).border_style(
        Style::default().fg(self.theme.accent),
    );
    let line = Line::from(vec![
        Span::raw(" "),
        left,
        Span::raw(" "),
        right,
    ]);
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_message_panel(&self, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" message ")
        .border_style(Style::default().fg(self.theme.accent));
    let inner = block.inner(area);
    if inner.height == 0 {
        frame.render_widget(block, area);
        return;
    }
    let lines = self.items_to_lines();
    let total = lines.len();
    let offset = if self.follow_bottom {
        total.saturating_sub(inner.height as usize)
    } else {
        (self.scroll as usize).min(total.saturating_sub(inner.height as usize))
    };
    self.scroll = offset as u16;
    let paragraph = Paragraph::new(Text::from(lines))
        .scroll((offset as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
    frame.render_widget(block, area);
}

fn render_status_panel(&self, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(self.theme.border));
    let prefix = if self.busy {
        Span::styled("[busy]", Style::default().fg(self.theme.warn))
    } else {
        Span::styled("[ok]", Style::default().fg(self.theme.success))
    };
    let style = if self.busy {
        Style::default().fg(self.theme.warn)
    } else {
        Style::default().fg(self.theme.muted)
    };
    let text = format!(" {} ", self.status);
    let line = Line::from(vec![
        prefix,
        Span::styled(text, style),
        Span::raw(" "),
        Span::styled(
            "/help · ctrl+c quit",
            Style::default().fg(self.theme.muted),
        ),
    ]);
    frame.render_widget(Paragraph::new(line).block(block), area);
}
```

- [ ] **Step 6: 新增 header_spans 辅助方法**

```rust
fn header_spans(&self) -> (Span<'static>, Span<'static>) {
    let state = if self.busy { " running " } else { " idle " };
    let state_style = if self.busy {
        Style::default().fg(self.theme.warn)
    } else {
        Style::default().fg(self.theme.success)
    };
    let left = Span::styled(
        format!(
            " pi-rust · {} · {} · mode {} · cwd {} · skills {} ",
            self.provider, self.model, self.mode_name, self.cwd, self.skills
        ),
        Style::default().fg(self.theme.muted),
    );
    let right = Span::styled(state, state_style);
    (left, right)
}
```

- [ ] **Step 7: 调整 render_status 状态色与 render_input 提示符色**

`render_status`(兜底无边框版)中 `self.busy` 用 `self.theme.warn`,否则 `self.theme.muted`。

`render_input`:

```rust
fn render_input(&self, frame: &mut Frame, area: Rect) {
    let prompt_style = if self.busy {
        Style::default().fg(self.theme.warn)
    } else {
        Style::default().fg(self.theme.accent)
    };
    let line = Line::from(vec![
        Span::styled("> ", prompt_style),
        Span::raw(self.input.clone()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
    let cursor_x = area.x + 2 + self.cursor as u16;
    let cursor_x = cursor_x.min(area.right().saturating_sub(1));
    frame.set_cursor_position(Position::new(cursor_x, area.y));
}
```

- [ ] **Step 8: 运行测试、clippy、fmt 并提交**

Run: `rtk cargo test -p pi-rust`
Run: `rtk cargo clippy -p pi-rust --all-targets -- -D warnings`
Run: `cargo fmt --check`
Expected: 全部通过 / 0 错误 / clean

```bash
git add examples/pi-rust/src/tui.rs
git commit -m "feat(pi-rust): TUI 布局 Panel 化,带边框面板与窄终端兜底"
```

---

### Task 3: 角色化渲染 + Meta 降噪 + 换行

**Files:**
- Modify: `examples/pi-rust/src/tui.rs`(consume_event ~391 行、items_to_lines ~594 行、tests 模块)
- Modify: `examples/pi-rust/src/render.rs`(删除对 Meta 事件输出的依赖;实际无需改——Meta 事件只由 TUI 消费,`render_event` 对 `ModelCallStart/End` 等在其他分支忽略)

**Interfaces:**
- Consumes: Task 1 的 `App.theme`, Task 2 的 Panel 布局
- Produces: 角色前缀(`you` 绿粗、`⋯` 品红斜体)、Meta 四事件降噪、`StreamText` 换行

- [ ] **Step 1: 新增 meta 降噪与角色渲染测试**

```rust
#[test]
fn meta_events_no_longer_produce_items() {
    let mut app = App::new(&config(), 0);
    for event in [
        AgentEvent::ModelCallStart(agent_scope_event::ModelCallStartEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
        }),
        AgentEvent::ModelCallEnd(agent_scope_event::ModelCallEndEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
        }),
        AgentEvent::ReplyStart(agent_scope_event::ReplyStartEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
        }),
        AgentEvent::ReplyEnd(agent_scope_event::ReplyEndEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
        }),
    ] {
        app.consume_event(event);
    }
    assert!(app.items.is_empty(), "Meta events must not produce UI items");
}

#[test]
fn user_message_prefix_and_thinking_style() {
    // 角色渲染在 items_to_lines 层实现,此处验证 items 内容本身。
    let mut app = App::new(&config(), 0);
    app.items.push(UiItem::UserMsg("hello".into()));
    app.consume_event(AgentEvent::ThinkingBlockDelta(
        agent_scope_event::ThinkingBlockDeltaEvent {
            base: agent_scope_event::EventBase::new(),
            reply_id: "r".into(),
            block_id: "t".into(),
            delta: "hmm".into(),
        },
    ));
    let lines = app.items_to_lines();
    // 第一行以 `you ` 前缀开头。
    assert!(lines[0].to_string().contains("you"), "{:?}", lines[0]);
    // 思考行含 `⋯` 与文本。
    assert!(lines[1].to_string().contains("⋯"), "{:?}", lines[1]);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `rtk cargo test -p pi-rust meta_events_no_longer_produce_items`
Expected: FAIL(`app.items` 非空,仍有 Meta item)

- [ ] **Step 3: consume_event 删除 Meta 分支**

删除 `consume_event` 中:

```rust
AgentEvent::ModelCallStart(_)
| AgentEvent::ModelCallEnd(_)
| AgentEvent::ReplyStart(_)
| AgentEvent::ReplyEnd(_) => {
    self.items
        .push(UiItem::Meta(event_name(&event).to_string()));
    self.follow_bottom = true;
}
```

替换为:

```rust
AgentEvent::ModelCallStart(_)
| AgentEvent::ModelCallEnd(_)
| AgentEvent::ReplyStart(_)
| AgentEvent::ReplyEnd(_) => {
    // 降噪:模型是否在工作由 header 状态点体现,工具活动由工具块呈现。
}
```

此时 `event_name` import 可能变为未使用——检查并删除 `event_name` 的 import(若 `items_to_lines` 的 `Meta` 分支仍引用 `event_name` 则保留;`items_to_lines` 中 `UiItem::Meta` 分支继续保留以兼容退化路径)。

- [ ] **Step 4: items_to_lines 角色化渲染**

将 `UiItem::UserMsg`、`UiItem::StreamText`、`UiItem::StreamThinking` 分支改造:

```rust
UiItem::UserMsg(text) => {
    lines.push(Line::from(vec![
        Span::styled(
            "you ",
            Style::default()
                .fg(self.theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(text.clone(), Style::default()),
    ]));
}
UiItem::StreamText(text) => {
    for sub in text.split('\n') {
        if !sub.is_empty() {
            lines.push(Line::from(sub.to_string()));
        }
    }
}
UiItem::StreamThinking(text) => {
    for sub in text.split('\n') {
        if !sub.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("⋯ ", Style::default().fg(self.theme.thinking)),
                Span::styled(
                    sub.to_string(),
                    Style::default()
                        .fg(self.theme.thinking)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }
    }
}
```

(StreamText 的换行由 Task 2 中 `render_message_panel` 的 `.wrap(Wrap { trim: false })` 提供;`UiItem::Meta` 分支保留但降为 `self.theme.muted`。)

- [ ] **Step 5: 运行全部测试、clippy、fmt 并提交**

Run: `rtk cargo test -p pi-rust`
Run: `rtk cargo clippy -p pi-rust --all-targets -- -D warnings`
Run: `cargo fmt --check`
Expected: 全部通过 / 0 错误 / clean

```bash
git add examples/pi-rust/src/tui.rs
git commit -m "feat(pi-rust): TUI 角色化渲染,Meta 事件降噪,长文本换行"
```

---

### Task 4: 综合验证 + 收尾

**Files:**
- Modify: `examples/pi-rust/src/tui.rs`(tests 模块:`theme_colors_are_consistent` 新测试)

**Interfaces:**
- Consumes: 全部前述 Task

- [ ] **Step 1: 新增 theme 一致性测试**

```rust
#[test]
fn theme_colors_are_consistent() {
    let theme = Theme::default();
    // 语义色必须互不相同,避免视觉混淆。
    let colors = [
        theme.accent, theme.success, theme.error, theme.warn, theme.muted, theme.thinking,
        theme.border,
    ];
    let mut unique = std::collections::HashSet::new();
    for c in colors {
        unique.insert(c);
    }
    assert_eq!(unique.len(), 7, "theme semantic colors must be distinct");
    // 语义:accent 主色、success/error/warn 为可辨识色。
    assert_ne!(theme.success, theme.error);
    assert_ne!(theme.accent, theme.muted);
}
```

- [ ] **Step 2: 运行测试确认通过**

Run: `rtk cargo test -p pi-rust theme_colors_are_consistent`
Expected: PASS

- [ ] **Step 3: 全量回归**

Run: `rtk cargo test -p pi-rust`
Run: `rtk cargo clippy -p pi-rust --all-targets -- -D warnings`
Run: `cargo fmt --check`
Run: `cargo doc -p pi-rust --no-deps`(确认无 broken intra-doc links)
Expected: 全部通过

- [ ] **Step 4: 手动目检**

在 TTY 中运行 `cargo run -p pi-rust -- --model <model>`,目检:
1. 面板边框 + `· message ·` 标题
2. 状态栏 `[ok]/[busy]` 前缀与右侧快捷键提示
3. 工具调用与结果缩进绑定
4. 长文本自动换行
5. 思考块品红斜体
6. `Ctrl+C` busy 时中断、idle 时退出;`/help` 覆盖层正常

- [ ] **Step 5: 提交收尾**

```bash
git add examples/pi-rust/src/tui.rs
git commit -m "test(pi-rust): TUI theme 一致性校验与最终回归"
```

---

## Self-Review

**Spec 覆盖核对:**
- Theme 集中配色 → Task 1 Step 3 / Task 4 Step 1 ✅
- 布局 Panel 化(Header/消息区边框、状态栏前缀、窄终端兜底)→ Task 2 ✅
- 角色化渲染 + 工具块绑定(you 前缀、⋯ 品红斜体、ToolCall 按 id 绑定 result)→ Task 1 Step 4-6 + Task 3 Step 4 ✅
- Meta 四事件降噪 → Task 3 Step 3 ✅
- 长文本自动换行 → Task 2 Step 5(`wrap(Wrap { trim: false })`)+ Task 3 Step 4 ✅
- 测试调整:更新 1 个现有测试 + 新增 4 个 → Task 1 Step 1/7、Task 2 Step 1、Task 3 Step 1、Task 4 Step 1 ✅

**类型一致性核对:**
- `UiItem::ToolCall` 四字段(`name`/`summary`/`tool_call_id`/`result`)在 Task 1 定义,Task 1 Step 6 与 Task 3 的 `items_to_lines` 匹配使用 `..` 省略 ✅
- `Theme` 七字段在 Task 1 定义,Task 2/3/4 全部按相同字段名取色 ✅
- `App::layout_areas(area: Rect) -> [Rect; 4]` 与 `App::bordered(area: Rect) -> bool`(均为关联函数,不借 `self`)在 Task 2 Step 1 测试与 Step 3 实现签名一致 ✅
- `App::header_spans(&self) -> (Span<'static>, Span<'static>)` 在 Task 2 Step 5/6 定义并使用 ✅

**注意(已在计划中体现):** `render_header` / `render_message_area` / `render_status` 三个旧方法保留作为窄终端兜底(Task 2 Step 4),避免无边框模式下绘制失败。`event_name` import 在 Task 3 Step 3 可能未使用,需按编译错误清理。
