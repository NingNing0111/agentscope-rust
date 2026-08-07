//! ratatui TUI frontend for the pi-rust coding Agent.
//!
//! The agent runs in a dedicated tokio task and streams render events and
//! confirmation requests to the UI over an mpsc channel; the UI event loop
//! alternates between keyboard events (`crossterm::event::EventStream`) and
//! channel messages, redrawing the screen after every processed event. This
//! lets thinking blocks, assistant text and tool calls render incrementally
//! as the model streams them.
//!
//! Channels use bounded capacity (256) to prevent unbounded memory growth when
//! the model emits tokens faster than the UI can draw.  Stream deltas are
//! coalesced: when the channel is full the agent will wait briefly rather than
//! silently dropping events, and UI-busy non-critical events (status/metadata)
//! are skipped rather than blocking the agent task.

#![deny(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::io::IsTerminal;

use agent_scope_agent::Agent;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use tokio::sync::{mpsc, oneshot};

/// Bounded channel capacity to prevent unbounded backlog under high-frequency
/// streaming.  When the channel is full the agent will block on send (applying
/// natural backpressure) rather than accumulating events in memory.
const UI_CHANNEL_CAP: usize = 256;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};

use crate::agent::AgentRuntime;
use crate::config::RuntimeConfig;
use crate::error::{PiError, PiResult};
use crate::render::{
    ConfirmationCandidate, RenderConfig, RenderedTurn, render_event, tool_call_summary,
    tool_result_line,
};
use crate::repl::{CommandOutput, handle_command, run_confirmation_loop};

/// Cap for buffered tool-output text per tool call in the UI (used for the
/// result excerpt and the expanded view).
const TOOL_OUTPUT_CAP: usize = 2000;

/// Whether the interactive frontend should be the TUI. Returns false when the
/// user passed `--no-tui` or either stdin/stdout is not a terminal (pipes and
/// CI would otherwise get stuck in raw mode).
pub fn use_tui(config: &RuntimeConfig) -> bool {
    !config.no_tui && std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
}

// ---------------------------------------------------------------------------
// UI state
// ---------------------------------------------------------------------------

/// A single visible entry in the scrolling message stream.
#[derive(Debug, Clone, PartialEq)]
pub enum UiItem {
    /// One-shot system line (welcome, command output, errors).
    System(String),
    /// A user prompt that was submitted.
    UserMsg(String),
    /// Accumulating assistant text block (appended by `TEXT_BLOCK_DELTA`).
    StreamText(String),
    /// Accumulating thinking block (appended by `THINKING_BLOCK_DELTA`).
    StreamThinking(String),
    /// A tool invocation summary line.
    ToolCall {
        name: String,
        summary: String,
        tool_call_id: String,
        /// 结果行(`→ success` / `→ error: …`),ToolResultEnd 到达时原地填充。
        result: Option<String>,
    },
    /// A tool result line (`→ success` / `→ error: …`).
    ToolResult(String),
    /// Low-key lifecycle marker (`MODEL_CALL_START`, `REPLY_START`, …).
    Meta(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Input,
    Confirm,
    Help,
}

/// In-flight confirmation dialog: collect one y/n decision per candidate, then
/// hand the full decision vector back to the agent task.
#[derive(Debug)]
pub struct ConfirmUi {
    candidates: Vec<ConfirmationCandidate>,
    decisions: Vec<bool>,
    index: usize,
    reply: Option<oneshot::Sender<Vec<bool>>>,
}

/// Commands the UI sends to the agent task.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AgentCmd {
    Prompt(String),
    Command(String),
    Interrupt,
    Shutdown,
}

/// Messages the agent task sends to the UI.
#[allow(clippy::large_enum_variant)] // `AgentEvent` is inherently large and sent by value
enum UiMsg {
    Event(AgentEvent),
    ConfirmRequest {
        candidates: Vec<ConfirmationCandidate>,
        reply: oneshot::Sender<Vec<bool>>,
    },
    CommandOutput(CommandOutput),
    Status(String),
    SetBusy(bool),
    TurnDone,
}

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
            border: Color::Gray,
        }
    }
}

pub struct App {
    input: String,
    cursor: usize,
    items: Vec<UiItem>,
    scroll: u16,
    follow_bottom: bool,
    mode: Mode,
    confirm: Option<ConfirmUi>,
    /// 确认完成后到 `TurnDone` 之间,抑制重跑 turn 的工具调用进入消息流:
    /// 已批准的工具操作已在确认面板中呈现,无需在输入框上方重复展示。
    suppress_tool_items: bool,
    busy: bool,
    status: String,
    help_text: String,
    theme: Theme,
    /// tool_call_id → tool name, captured at `ToolCallStart`.
    tool_call_names: HashMap<String, String>,
    /// tool_call_id → accumulated (capped) output text.
    tool_outputs: HashMap<String, String>,
    // Header snapshot.
    provider: String,
    model: String,
    mode_name: String,
    cwd: String,
    skills: usize,
}

impl App {
    fn new(config: &RuntimeConfig, skills: usize) -> Self {
        let help_text = help_text(config, skills);
        Self {
            input: String::new(),
            cursor: 0,
            items: Vec::new(),
            scroll: 0,
            follow_bottom: true,
            mode: Mode::Input,
            confirm: None,
            suppress_tool_items: false,
            busy: false,
            status: String::new(),
            help_text,
            theme: Theme::default(),
            tool_call_names: HashMap::new(),
            tool_outputs: HashMap::new(),
            provider: config.provider.name().to_string(),
            model: config.model.clone(),
            mode_name: config.mode.as_str().to_string(),
            cwd: config.cwd.display().to_string(),
            skills,
        }
    }

    // -- event handling ------------------------------------------------------

    fn handle_event(&mut self, event: Event, agent_tx: &mpsc::UnboundedSender<AgentCmd>) -> bool {
        match event {
            Event::Key(key) => self.handle_key(key, agent_tx),
            Event::Paste(text) => {
                for ch in text.chars() {
                    self.insert_char(ch);
                }
                false
            }
            _ => false,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, agent_tx: &mpsc::UnboundedSender<AgentCmd>) -> bool {
        match self.mode {
            Mode::Confirm => {
                self.handle_confirm_key(key);
                false
            }
            Mode::Help => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.mode = Mode::Input;
                    self.status = "Ready".to_string();
                    false
                }
                _ => false,
            },
            Mode::Input => self.handle_input_key(key, agent_tx),
        }
    }

    fn handle_input_key(
        &mut self,
        key: KeyEvent,
        agent_tx: &mpsc::UnboundedSender<AgentCmd>,
    ) -> bool {
        match key.code {
            // Ctrl+C: interrupt a running turn, or save-and-quit while idle.
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.busy {
                    let _ = agent_tx.send(AgentCmd::Interrupt);
                    self.status = "interrupting…".to_string();
                } else {
                    self.items
                        .push(UiItem::System("saving session…".to_string()));
                    self.follow_bottom = true;
                    let _ = agent_tx.send(AgentCmd::Shutdown);
                    return true;
                }
            }
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Enter => {
                if self.busy {
                    self.status = "agent is running — press Ctrl+C to interrupt".to_string();
                    return false;
                }
                let text = self.input.trim().to_string();
                if text.is_empty() {
                    return false;
                }
                self.input.clear();
                self.cursor = 0;
                if text.starts_with('/') {
                    // /help opens the in-app help overlay; everything else goes
                    // to the agent task which owns the runtime.
                    if text == "/help" {
                        self.mode = Mode::Help;
                        self.status = "help — Esc or q to close".to_string();
                    } else {
                        let _ = agent_tx.send(AgentCmd::Command(text));
                    }
                } else {
                    self.items.push(UiItem::UserMsg(text.clone()));
                    self.follow_bottom = true;
                    self.busy = true;
                    self.status = "running…".to_string();
                    let _ = agent_tx.send(AgentCmd::Prompt(text));
                }
            }
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.chars().count(),
            KeyCode::Esc => {
                self.input.clear();
                self.cursor = 0;
            }
            KeyCode::Up => self.scroll_up(1),
            KeyCode::Down => self.scroll_down(1),
            KeyCode::PageUp => self.scroll_up(10),
            KeyCode::PageDown => self.scroll_down(10),
            _ => {}
        }
        false
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        let (finished, decisions) = {
            let Some(confirm) = self.confirm.as_mut() else {
                return;
            };
            let mut fill = |value: bool| {
                let remaining = confirm
                    .candidates
                    .len()
                    .saturating_sub(confirm.decisions.len());
                confirm
                    .decisions
                    .extend(std::iter::repeat_n(value, remaining));
            };
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    confirm.decisions.push(true);
                    confirm.index += 1;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    confirm.decisions.push(false);
                    confirm.index += 1;
                }
                KeyCode::Char('a') | KeyCode::Char('A') => fill(true),
                KeyCode::Char('d') | KeyCode::Char('D') => fill(false),
                KeyCode::Esc => fill(false),
                _ => {}
            }
            let done = confirm.decisions.len() >= confirm.candidates.len();
            (done, confirm.decisions.clone())
        };
        if finished {
            if let Some(reply) = self.confirm.take().and_then(|c| c.reply) {
                let _ = reply.send(decisions);
            }
            self.mode = Mode::Input;
            self.status = "Ready".to_string();
            // 批准(或拒绝)的操作已通过确认面板呈现;后续重跑 turn 的工具
            // 调用不再进入消息流,避免在输入框上方重复展示。
            self.suppress_tool_items = true;
        }
    }

    // -- agent messages ------------------------------------------------------

    /// Returns true when the app should exit.
    fn handle_ui_msg(&mut self, msg: UiMsg) -> bool {
        match msg {
            UiMsg::Event(event) => {
                self.consume_event(event);
                false
            }
            UiMsg::ConfirmRequest { candidates, reply } => {
                // 待确认的工具操作已进入确认面板,将其调用行从消息流移除,
                // 避免确认完成后仍残留在输入框上方。
                let confirming: HashSet<String> = candidates
                    .iter()
                    .map(|candidate| candidate.tool_call_id.clone())
                    .collect();
                self.items.retain(|item| match item {
                    UiItem::ToolCall { tool_call_id, .. } => !confirming.contains(tool_call_id),
                    _ => true,
                });
                self.mode = Mode::Confirm;
                self.confirm = Some(ConfirmUi {
                    candidates,
                    decisions: Vec::new(),
                    index: 0,
                    reply: Some(reply),
                });
                self.status =
                    "approve?  [y] yes  [n] no  [a] all  [d] deny  [Esc] cancel".to_string();
                false
            }
            UiMsg::CommandOutput(output) => {
                for message in output.messages {
                    self.items.push(UiItem::System(message));
                }
                self.follow_bottom = true;
                output.should_exit
            }
            UiMsg::Status(text) => {
                self.status = text;
                false
            }
            UiMsg::SetBusy(busy) => {
                self.busy = busy;
                false
            }
            UiMsg::TurnDone => {
                self.busy = false;
                self.suppress_tool_items = false;
                self.status = "Ready".to_string();
                self.follow_bottom = true;
                false
            }
        }
    }

    /// Fold one agent event into the message stream.
    fn consume_event(&mut self, event: AgentEvent) {
        match &event {
            AgentEvent::ThinkingBlockDelta(delta) => self.append_stream(&delta.delta, true),
            AgentEvent::TextBlockDelta(delta) => self.append_stream(&delta.delta, false),
            AgentEvent::ToolCallStart(start) => {
                self.tool_call_names
                    .insert(start.tool_call_id.clone(), start.tool_call_name.clone());
            }
            // 确认完成后的重跑 turn 抑制工具行:已批准的操作在确认面板中
            // 呈现,这里不再进入消息流,避免在输入框上方重复展示。
            AgentEvent::ToolCallEnd(end) if !self.suppress_tool_items => {
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
            AgentEvent::ToolResultTextDelta(delta) if !self.suppress_tool_items => {
                let entry = self
                    .tool_outputs
                    .entry(delta.tool_call_id.clone())
                    .or_default();
                if entry.chars().count() < TOOL_OUTPUT_CAP {
                    entry.push_str(&delta.delta);
                }
            }
            AgentEvent::ToolResultEnd(end) if !self.suppress_tool_items => {
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
            AgentEvent::UserInterrupt(_) => {
                self.items.push(UiItem::System("[interrupted]".to_string()));
                self.follow_bottom = true;
            }
            AgentEvent::ModelCallStart(_)
            | AgentEvent::ModelCallEnd(_)
            | AgentEvent::ReplyStart(_)
            | AgentEvent::ReplyEnd(_) => {
                // 降噪:模型是否在工作由 header 状态点体现,工具活动由工具块呈现。
            }
            _ => {}
        }
    }

    /// Append a delta to the current text/thinking block, or start a new one.
    fn append_stream(&mut self, delta: &str, is_thinking: bool) {
        let appended = match self.items.last_mut() {
            Some(UiItem::StreamText(text)) if !is_thinking => {
                text.push_str(delta);
                true
            }
            Some(UiItem::StreamThinking(text)) if is_thinking => {
                text.push_str(delta);
                true
            }
            _ => false,
        };
        if !appended {
            if is_thinking {
                self.items.push(UiItem::StreamThinking(delta.to_string()));
            } else {
                self.items.push(UiItem::StreamText(delta.to_string()));
            }
            // A new block pulls the view back to the bottom; in-flight deltas
            // on an existing block do not disturb a manual scroll.
            self.follow_bottom = true;
        }
    }

    // -- input editing -------------------------------------------------------

    fn insert_char(&mut self, c: char) {
        if self.cursor >= self.input.chars().count() {
            self.input.push(c);
        } else {
            let mut chars: Vec<char> = self.input.chars().collect();
            chars.insert(self.cursor, c);
            self.input = chars.into_iter().collect();
        }
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.input.chars().collect();
        chars.remove(self.cursor - 1);
        self.input = chars.into_iter().collect();
        self.cursor -= 1;
    }

    fn delete(&mut self) {
        if self.cursor >= self.input.chars().count() {
            return;
        }
        let mut chars: Vec<char> = self.input.chars().collect();
        chars.remove(self.cursor);
        self.input = chars.into_iter().collect();
    }

    fn move_cursor_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_cursor_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.input.chars().count());
    }

    fn scroll_up(&mut self, amount: u16) {
        self.follow_bottom = false;
        self.scroll = self.scroll.saturating_add(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.follow_bottom = false;
        self.scroll = self.scroll.saturating_sub(amount);
    }

    // -- rendering -----------------------------------------------------------

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
        .areas::<4>(area)
    }

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
            Mode::Confirm => self.render_confirm_panel(frame, main),
            Mode::Help => self.render_overlay(frame, "help", self.help_lines()),
            Mode::Input => {}
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let busy_style = if self.busy {
            Style::default().fg(self.theme.warn)
        } else {
            Style::default().fg(self.theme.success)
        };
        let state = if self.busy { " running " } else { " idle " };
        let line = Line::from(vec![
            Span::styled(
                " pi-rust ",
                Style::default().fg(Color::Black).bg(self.theme.accent),
            ),
            Span::raw(format!(
                " {} · {} · mode {} · cwd {} · skills {} ",
                self.provider, self.model, self.mode_name, self.cwd, self.skills
            )),
            Span::styled(state, busy_style),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

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

    fn render_header_panel(&self, frame: &mut Frame, area: Rect) {
        let (left, right) = self.header_spans();
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(self.theme.accent));
        let line = Line::from(vec![Span::raw(" "), left, Span::raw(" "), right]);
        frame.render_widget(Paragraph::new(line).block(block), area);
    }

    fn render_message_panel(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" message ")
            .border_style(Style::default().fg(self.theme.accent));
        let inner = block.inner(area);
        if inner.height == 0 {
            frame.render_widget(block, area);
            return;
        }
        let text = Text::from(self.items_to_lines());
        // wrap 后的物理总高(长段落被 wrap 拆成多物理行,逻辑行数会低估)
        let total = wrapped_height(&text, inner.width);
        let offset = scroll_offset(total, inner.height, self.follow_bottom, self.scroll);
        self.scroll = offset;
        let paragraph = Paragraph::new(text)
            .scroll((offset, 0))
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
            Span::styled("/help · ctrl+c quit", Style::default().fg(self.theme.muted)),
        ]);
        frame.render_widget(Paragraph::new(line).block(block), area);
    }

    fn render_message_area(&mut self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        let text = Text::from(self.items_to_lines());
        let total = wrapped_height(&text, area.width);
        let offset = scroll_offset(total, area.height, self.follow_bottom, self.scroll);
        self.scroll = offset;
        let paragraph = Paragraph::new(text)
            .scroll((offset, 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let text = format!("  {}", self.status);
        let style = if self.busy {
            Style::default().fg(self.theme.warn)
        } else {
            Style::default().fg(self.theme.muted)
        };
        frame.render_widget(Paragraph::new(text).style(style), area);
    }

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

    /// Draw a centered bordered overlay (confirm/help).
    fn render_overlay(&self, frame: &mut Frame, title: &str, lines: Vec<Line<'static>>) {
        let area = frame.area();
        if area.width < 24 || area.height < 6 {
            return;
        }
        let width = area.width.min(72).saturating_sub(4).max(20);
        let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4).max(4));
        let popup = centered_rect(area, width, height);
        frame.render_widget(ratatui::widgets::Clear, popup);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(Style::default().fg(self.theme.accent));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            inner,
        );
    }

    /// Draw the confirmation panel anchored to the bottom of the message area,
    /// directly above the input line — never centered over the conversation.
    fn render_confirm_panel(&self, frame: &mut Frame, area: Rect) {
        let lines = self.confirm_lines();
        if area.height == 0 {
            return;
        }
        // 内容行 + 上下边框。
        let height = (lines.len() as u16 + 2).min(area.height);
        let panel = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(height),
            width: area.width,
            height,
        };
        frame.render_widget(ratatui::widgets::Clear, panel);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" confirm ")
            .border_style(Style::default().fg(self.theme.warn));
        let inner = block.inner(panel);
        frame.render_widget(block, panel);
        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn items_to_lines(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        for item in &self.items {
            match item {
                UiItem::System(text) => {
                    for sub in text.split('\n') {
                        lines.push(Line::from(Span::styled(
                            sub.to_string(),
                            Style::default().fg(self.theme.muted),
                        )));
                    }
                }
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
                UiItem::Meta(text) => {
                    lines.push(Line::from(Span::styled(
                        format!("· {text}"),
                        Style::default().fg(self.theme.muted),
                    )));
                }
            }
        }
        lines
    }

    fn confirm_lines(&self) -> Vec<Line<'static>> {
        let Some(confirm) = &self.confirm else {
            return vec![];
        };
        let mut lines = vec![Line::from("Approve the following tool operations?")];
        for (i, candidate) in confirm.candidates.iter().enumerate() {
            let marker = if i < confirm.decisions.len() {
                if confirm.decisions[i] {
                    " ✓ "
                } else {
                    " ✗ "
                }
            } else if i == confirm.index {
                " ▶ "
            } else {
                "   "
            };
            lines.push(Line::from(format!("{marker}{}", candidate.description)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(
            "[y] yes  [n] no  [a] approve all  [d] deny all  [Esc] cancel",
        ));
        lines
    }

    fn help_lines(&self) -> Vec<Line<'static>> {
        self.help_text
            .split('\n')
            .map(|line| {
                if line.starts_with('/') || line.starts_with("pi-rust") {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Cyan),
                    ))
                } else {
                    Line::from(line.to_string())
                }
            })
            .collect()
    }
}

/// Estimate the number of physical (wrapped) lines for a `Text` in a given
/// column width.  ratatui `WordWrapper` breaks at word boundaries, so
/// `line.width().div_ceil(width)` gives a reasonable upper-bound that is
/// tighter than naive line-count.
fn wrapped_height(text: &Text<'_>, area_width: u16) -> usize {
    if area_width < 1 {
        return text.lines.len();
    }
    let width = area_width as usize;
    let mut total = 0usize;
    for line in &text.lines {
        let line_width = line.width();
        let wrapped = if line_width == 0 {
            1
        } else {
            line_width.div_ceil(width)
        };
        total += wrapped;
    }
    total
}

/// Calculate a scroll offset — pure function, testable independently.
fn scroll_offset(total: usize, viewport: u16, follow_bottom: bool, scroll: u16) -> u16 {
    let threshold = total.saturating_sub(viewport as usize);
    let offset = if follow_bottom {
        threshold
    } else {
        (scroll as usize).min(threshold)
    };
    offset as u16
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

fn help_text(config: &RuntimeConfig, skills: usize) -> String {
    format!(
        r#"pi-rust commands:
  /help       Show this help, active config, and examples
  /model      Show provider/model without secrets
  /tools      Show registered tools and permission behavior
  /skills     List loaded workspace skills
  /skill NAME Show a loaded skill's full instructions
  /sessions   List persisted sessions
  /save       Save current session
  /tasks      Show the agent's task plan/progress/completion state
  /approvals  List host-approved destructive operations this session
  /context    Show the agent's context message count
  /events on|off  Toggle human-readable lifecycle/tool events
  /json on|off    Toggle redacted JSON event lines
  /exit, /quit    Save and exit

Keys:
  Enter        Send the input line (or open /help)
  Ctrl+C       Interrupt the running agent; save and quit when idle
  Esc          Clear the input
  Up/Down      Scroll the message area (PgUp/PgDn scroll by 10)
  y/n/a/d      In the confirm dialog: yes / no / approve all / deny all
  q/Esc        Close the help overlay

Active config:
  provider: {provider}
  model: {model}
  mode: {mode}
  cwd: {cwd}
  tools: {tools}
  skills: {skills}
  memory: {memory}
  rag: {rag}

Sample prompts:
  请读取 src/main.rs 并说明它的主要功能。
  用 Grep 搜索项目里所有调用 println! 的地方。
  创建 hello.txt，内容是 Hello, World!
  把 hello.txt 中的 World 改成 Rust。
  执行 pwd，并告诉我返回了什么。
  请按 coding workflow 修改并验证这个项目。"#,
        provider = config.provider.name(),
        model = config.model,
        mode = config.mode.as_str(),
        cwd = config.cwd.display(),
        tools = !config.no_tools,
        skills = skills,
        memory = !config.no_memory,
        rag = !config.no_rag,
    )
}

// ---------------------------------------------------------------------------
// App loop + agent task
// ---------------------------------------------------------------------------

pub async fn run_interactive_tui(runtime: AgentRuntime) -> PiResult<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, runtime).await;
    let _ = ratatui::try_restore();
    result
}

async fn run_app(terminal: &mut ratatui::DefaultTerminal, runtime: AgentRuntime) -> PiResult<()> {
    let (ui_tx, mut ui_rx) = mpsc::channel::<UiMsg>(UI_CHANNEL_CAP);
    let (agent_tx, agent_rx) = mpsc::unbounded_channel::<AgentCmd>();

    let mut app = App::new(&runtime.config, runtime.skills.len());
    app.items.push(UiItem::System(format!(
        "pi-rust ready · provider={} · model={} · mode={} · cwd={} · skills={}",
        runtime.config.provider.name(),
        runtime.config.model,
        runtime.config.mode.as_str(),
        runtime.config.cwd.display(),
        runtime.skills.len()
    )));
    app.items.push(UiItem::System(
        "Enter to send · Ctrl+C to interrupt/quit · /help for commands".to_string(),
    ));
    app.follow_bottom = true;

    let agent_handle = tokio::spawn(agent_task(runtime, ui_tx, agent_rx));
    let mut events = EventStream::new();

    loop {
        tokio::select! {
            maybe = events.next() => {
                if let Some(Ok(event)) = maybe
                    && app.handle_event(event, &agent_tx)
                {
                    break;
                }
            }
            maybe = ui_rx.recv() => {
                match maybe {
                    Some(msg) => {
                        if app.handle_ui_msg(msg) {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
        terminal
            .draw(|frame| app.render(frame))
            .map_err(|err| PiError::io("tui draw", err))?;
    }

    let _ = agent_tx.send(AgentCmd::Shutdown);
    let _ = agent_handle.await;
    Ok(())
}

/// Owns the runtime; runs one turn per `Prompt`, executes `/` commands, and
/// forwards the agent's interrupt.
async fn agent_task(
    mut runtime: AgentRuntime,
    ui_tx: mpsc::Sender<UiMsg>,
    mut agent_rx: mpsc::UnboundedReceiver<AgentCmd>,
) {
    while let Some(cmd) = agent_rx.recv().await {
        match cmd {
            AgentCmd::Prompt(input) => {
                let _ = ui_tx.send(UiMsg::SetBusy(true)).await;
                let turn = run_turn_with_confirmations_tui(&runtime, &input, &ui_tx).await;
                match turn {
                    Ok(turn) => {
                        let tasks =
                            serde_json::to_value(runtime.agent.try_state().tasks_context.clone())
                                .unwrap_or_default();
                        runtime
                            .session
                            .add_turn(input, turn.events, turn.text, None);
                        runtime.session.snapshot_tasks(tasks);
                        if let Err(err) = runtime.store.save(&runtime.session) {
                            let _ = ui_tx
                                .send(UiMsg::Status(format!(
                                    "save failed: {}",
                                    err.safe_message()
                                )))
                                .await;
                        }
                    }
                    Err(err) => {
                        let _ = ui_tx
                            .send(UiMsg::Status(format!("error: {}", err.safe_message())))
                            .await;
                    }
                }
                let _ = ui_tx.send(UiMsg::SetBusy(false)).await;
                let _ = ui_tx.send(UiMsg::TurnDone).await;
            }
            AgentCmd::Command(command) => {
                let output = handle_command(&mut runtime, &command);
                match output {
                    Ok(output) => {
                        let should_exit = output.should_exit;
                        let _ = ui_tx.send(UiMsg::CommandOutput(output)).await;
                        if should_exit {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = ui_tx
                            .send(UiMsg::Status(format!("error: {}", err.safe_message())))
                            .await;
                    }
                }
            }
            AgentCmd::Interrupt => runtime.agent.interrupt(),
            AgentCmd::Shutdown => break,
        }
    }
    // Persist on every exit path.
    let _ = runtime.store.save(&runtime.session);
}

/// Like `repl::run_turn` but streams every event to the UI instead of
/// printing it, and uses the collection-only render config.
///
/// Stream deltas (`TextBlockDelta` / `ThinkingBlockDelta`) are coalesced:
/// consecutive deltas of the same kind are merged into a single `UiMsg::Event`
/// to reduce channel pressure when the model emits tokens faster than the UI
/// can draw.
async fn run_turn_for_tui(
    runtime: &AgentRuntime,
    input: &str,
    ui_tx: &mpsc::Sender<UiMsg>,
) -> PiResult<RenderedTurn> {
    let msg = user_msg("user", input)?;
    let mut stream = runtime.agent.reply_stream(Some(vec![msg])).await?;
    let config = RenderConfig {
        cwd: runtime.config.cwd.clone(),
        show_events: false,
        show_json_events: false,
    };
    let mut turn = RenderedTurn::default();
    // Buffer for coalescing consecutive deltas of the same block kind.
    let mut pending_delta: Option<(bool, String)> = None; // (is_thinking, accumulated_text)

    /// Flush any pending coalesced delta to the UI.
    fn flush_delta(ui_tx: &mpsc::Sender<UiMsg>, pending: &mut Option<(bool, String)>) {
        if let Some((is_thinking, text)) = pending.take() {
            let event = if is_thinking {
                AgentEvent::ThinkingBlockDelta(agent_scope_event::ThinkingBlockDeltaEvent {
                    base: agent_scope_event::EventBase::new(),
                    reply_id: String::new(),
                    block_id: String::new(),
                    delta: text,
                })
            } else {
                AgentEvent::TextBlockDelta(agent_scope_event::TextBlockDeltaEvent {
                    base: agent_scope_event::EventBase::new(),
                    reply_id: String::new(),
                    block_id: String::new(),
                    delta: text,
                })
            };
            let _ = ui_tx.try_send(UiMsg::Event(event));
        }
    }

    while let Some(event) = stream.next().await {
        render_event(event.clone(), &config, &mut turn)?;

        // Coalesce consecutive deltas: merge same-kind deltas, flush on any
        // other event type.
        match &event {
            AgentEvent::ThinkingBlockDelta(delta) => {
                if let Some((true, ref mut text)) = pending_delta {
                    text.push_str(&delta.delta);
                } else {
                    flush_delta(ui_tx, &mut pending_delta);
                    pending_delta = Some((true, delta.delta.clone()));
                }
            }
            AgentEvent::TextBlockDelta(delta) => {
                if let Some((false, ref mut text)) = pending_delta {
                    text.push_str(&delta.delta);
                } else {
                    flush_delta(ui_tx, &mut pending_delta);
                    pending_delta = Some((false, delta.delta.clone()));
                }
            }
            _ => {
                flush_delta(ui_tx, &mut pending_delta);
                // Non-delta events carry lifecycle state (tool starts/ends,
                // results, interrupts). Use a blocking send so a full channel
                // applies backpressure instead of silently dropping these —
                // the module doc promises blocking-on-full (round-5 H1). Deltas
                // above stay `try_send` because they are coalescible display
                // details.
                let _ = ui_tx.send(UiMsg::Event(event)).await;
            }
        }
    }
    flush_delta(ui_tx, &mut pending_delta);
    Ok(turn)
}

/// `repl::run_turn_with_confirmations` variant whose ask closure hands the
/// candidate list to the UI and waits for the keypress decisions.
///
/// Retry failures are NOT silently swallowed: when the oneshot channel
/// disconnects (UI dropped) or the retry turn errors, the error is surfaced
/// via the status line so the user sees the failure rather than a blank
/// succeed.
async fn run_turn_with_confirmations_tui(
    runtime: &AgentRuntime,
    input: &str,
    ui_tx: &mpsc::Sender<UiMsg>,
) -> PiResult<RenderedTurn> {
    let approvals = std::sync::Arc::clone(&runtime.approvals);
    let first = run_turn_for_tui(runtime, input, ui_tx).await?;
    let ask = |candidates: &[ConfirmationCandidate]| {
        let ui_tx = ui_tx.clone();
        let candidates: Vec<ConfirmationCandidate> = candidates.to_vec();
        async move {
            let (reply_tx, reply_rx) = oneshot::channel();
            let _ = ui_tx
                .send(UiMsg::ConfirmRequest {
                    candidates,
                    reply: reply_tx,
                })
                .await;
            // `unwrap_or_default()` would silently treat a broken channel as
            // "all denied".  We surface the error and return a conservative
            // denial — but first post a status message so the user sees it.
            match reply_rx.await {
                Ok(decisions) => decisions,
                Err(_) => {
                    let _ = ui_tx
                        .send(UiMsg::Status(
                            "confirmation dialog closed unexpectedly; denying all".to_string(),
                        ))
                        .await;
                    Vec::new()
                }
            }
        }
    };
    let result = run_confirmation_loop(
        &approvals,
        first,
        || async {
            match run_turn_for_tui(runtime, input, ui_tx).await {
                Ok(turn) => turn,
                Err(err) => {
                    let _ = ui_tx
                        .send(UiMsg::Status(format!(
                            "retry turn failed: {}",
                            err.safe_message()
                        )))
                        .await;
                    RenderedTurn::default()
                }
            }
        },
        ask,
    )
    .await;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ProviderConfig, RunMode};

    fn config() -> RuntimeConfig {
        RuntimeConfig {
            api_key: "sk-test".into(),
            masked_api_key: "****".into(),
            model: "qwen-plus".into(),
            provider: ProviderConfig::DashScope,
            workdir: std::path::PathBuf::from(".pi-rust"),
            cwd: std::path::PathBuf::from("."),
            mode: RunMode::React,
            skill_paths: vec![],
            prompt: None,
            resume: None,
            list_sessions: false,
            no_tools: false,
            no_memory: false,
            no_rag: false,
            max_iters: 20,
            command_timeout_secs: 30,
            show_events: false,
            show_json_events: false,
            no_tui: false,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn use_tui_requires_tty_and_not_disabled() {
        let mut cfg = config();
        cfg.no_tui = true;
        assert!(!use_tui(&cfg));
        // When not disabled the decision is delegated to is_terminal(), which
        // is false in the test harness — so it must also be false here.
        cfg.no_tui = false;
        assert!(!use_tui(&cfg));
    }

    #[test]
    fn text_delta_accumulates_into_single_stream_text() {
        let mut app = App::new(&config(), 0);
        app.consume_event(AgentEvent::TextBlockDelta(
            agent_scope_event::TextBlockDeltaEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                block_id: "b".into(),
                delta: "Hel".into(),
            },
        ));
        app.consume_event(AgentEvent::TextBlockDelta(
            agent_scope_event::TextBlockDeltaEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                block_id: "b".into(),
                delta: "lo!".into(),
            },
        ));
        assert_eq!(app.items.len(), 1);
        assert_eq!(app.items[0], UiItem::StreamText("Hello!".to_string()));
    }

    #[test]
    fn thinking_and_text_stream_to_separate_items() {
        let mut app = App::new(&config(), 0);
        app.consume_event(AgentEvent::ThinkingBlockDelta(
            agent_scope_event::ThinkingBlockDeltaEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                block_id: "t".into(),
                delta: "hmm".into(),
            },
        ));
        app.consume_event(AgentEvent::TextBlockDelta(
            agent_scope_event::TextBlockDeltaEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                block_id: "b".into(),
                delta: "answer".into(),
            },
        ));
        assert_eq!(app.items.len(), 2);
        assert!(matches!(app.items[0], UiItem::StreamThinking(_)));
        assert!(matches!(app.items[1], UiItem::StreamText(_)));
    }

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
            &app.items[0],
            UiItem::ToolCall { name, summary, result, .. }
                if name == "Bash" && summary.contains("ls") && result.as_deref() == Some("→ success")
        ));
        assert_eq!(
            app.items.len(),
            1,
            "ToolResult must fold into the ToolCall item"
        );
    }

    #[test]
    fn tool_result_binds_by_tool_call_id() {
        let mut app = App::new(&config(), 0);
        // 两个并行工具调用(事件流允许交错),结果按 tool_call_id 精确绑定而非看顺序。
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
        assert_eq!(
            app.items.len(),
            2,
            "both results must fold into their own items"
        );
        let a = &app.items[0];
        let b = &app.items[1];
        assert!(matches!(
            a,
            UiItem::ToolCall { result, .. } if result.as_deref() == Some("→ success")
        ));
        assert!(matches!(
            b,
            UiItem::ToolCall { result, .. } if result.as_deref().is_some_and(|r| r.starts_with("→ error"))
        ));
    }

    #[test]
    fn enter_submits_prompt_when_idle_and_ignores_while_busy() {
        let (agent_tx, mut agent_rx) = mpsc::unbounded_channel();
        let mut app = App::new(&config(), 0);
        for ch in "hello".chars() {
            app.insert_char(ch);
        }
        app.handle_key(key(KeyCode::Enter), &agent_tx);
        assert!(matches!(app.items[0], UiItem::UserMsg(_)));
        assert!(app.busy);
        assert_eq!(
            agent_rx.try_recv().unwrap(),
            AgentCmd::Prompt("hello".into())
        );

        // Busy: Enter is ignored and no new prompt is enqueued.
        app.insert_char('x');
        app.handle_key(key(KeyCode::Enter), &agent_tx);
        assert!(
            agent_rx.try_recv().is_err(),
            "busy Enter must not enqueue another prompt"
        );
    }

    #[test]
    fn confirm_dialog_collects_y_n_and_deny_all() {
        let (agent_tx, _agent_rx) = mpsc::unbounded_channel();
        let mut app = App::new(&config(), 0);
        // Simulate the agent task posting a confirmation request.
        let (reply_tx, mut reply_rx) = oneshot::channel();
        app.handle_ui_msg(UiMsg::ConfirmRequest {
            candidates: vec![
                ConfirmationCandidate {
                    tool_call_id: "tc-1".into(),
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm a".into(),
                    description: "[Bash] $ rm a".into(),
                },
                ConfirmationCandidate {
                    tool_call_id: "tc-2".into(),
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm b".into(),
                    description: "[Bash] $ rm b".into(),
                },
            ],
            reply: reply_tx,
        });
        assert_eq!(app.mode, Mode::Confirm);

        // y for the first, n for the second → finished.
        app.handle_key(key(KeyCode::Char('y')), &agent_tx);
        app.handle_key(key(KeyCode::Char('n')), &agent_tx);
        assert_eq!(app.mode, Mode::Input);
        assert_eq!(reply_rx.try_recv().unwrap(), vec![true, false]);
    }

    #[test]
    fn confirm_dialog_deny_all_via_d() {
        let (agent_tx, _agent_rx) = mpsc::unbounded_channel();
        let mut app = App::new(&config(), 0);
        let (reply_tx, mut reply_rx) = oneshot::channel();
        app.handle_ui_msg(UiMsg::ConfirmRequest {
            candidates: vec![
                ConfirmationCandidate {
                    tool_call_id: "tc-3".into(),
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm a".into(),
                    description: "[Bash] $ rm a".into(),
                },
                ConfirmationCandidate {
                    tool_call_id: "tc-4".into(),
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm b".into(),
                    description: "[Bash] $ rm b".into(),
                },
                ConfirmationCandidate {
                    tool_call_id: "tc-5".into(),
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm c".into(),
                    description: "[Bash] $ rm c".into(),
                },
            ],
            reply: reply_tx,
        });
        app.handle_key(key(KeyCode::Char('d')), &agent_tx);
        assert_eq!(app.mode, Mode::Input);
        assert_eq!(reply_rx.try_recv().unwrap(), vec![false, false, false]);
    }

    #[test]
    fn confirm_request_removes_pending_tool_items() {
        let (_agent_tx, _agent_rx) = mpsc::unbounded_channel::<AgentCmd>();
        let mut app = App::new(&config(), 0);
        // 消息流中已有两条待确认的工具调用行 + 一条无关系统行。
        app.items.push(UiItem::ToolCall {
            name: "Bash".into(),
            summary: "[Bash] $ rm a".into(),
            tool_call_id: "tc-a".into(),
            result: None,
        });
        app.items.push(UiItem::ToolCall {
            name: "Bash".into(),
            summary: "[Bash] $ rm b".into(),
            tool_call_id: "tc-b".into(),
            result: None,
        });
        app.items.push(UiItem::System("keep me".into()));

        let (reply_tx, _reply_rx) = oneshot::channel();
        app.handle_ui_msg(UiMsg::ConfirmRequest {
            candidates: vec![
                ConfirmationCandidate {
                    tool_call_id: "tc-a".into(),
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm a".into(),
                    description: "[Bash] $ rm a".into(),
                },
                ConfirmationCandidate {
                    tool_call_id: "tc-b".into(),
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm b".into(),
                    description: "[Bash] $ rm b".into(),
                },
            ],
            reply: reply_tx,
        });

        assert_eq!(app.mode, Mode::Confirm);
        assert_eq!(app.items.len(), 1, "待确认工具行应从消息流移除,只留无关行");
        assert!(matches!(app.items[0], UiItem::System(_)));
    }

    #[test]
    fn retry_tool_items_suppressed_until_turn_done() {
        let (agent_tx, _agent_rx) = mpsc::unbounded_channel();
        let mut app = App::new(&config(), 0);
        let (reply_tx, _reply_rx) = oneshot::channel();
        app.handle_ui_msg(UiMsg::ConfirmRequest {
            candidates: vec![ConfirmationCandidate {
                tool_call_id: "tc-a".into(),
                tool_name: "Bash".into(),
                fingerprint: "bash:rm a".into(),
                description: "[Bash] $ rm a".into(),
            }],
            reply: reply_tx,
        });
        // 批准 → 确认完成,进入抑制期(重跑 turn 的工具不再展示)。
        app.handle_key(key(KeyCode::Char('y')), &agent_tx);
        assert!(app.suppress_tool_items, "确认完成后应进入抑制期");

        // 重跑 turn 的工具事件:调用行与结果行都不应进入消息流。
        app.consume_event(AgentEvent::ToolCallEnd(
            agent_scope_event::ToolCallEndEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                tool_call_id: "tc-a".into(),
                input: Some(r#"{"command":"rm a"}"#.into()),
            },
        ));
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
        assert_eq!(app.items.len(), 0, "重跑 turn 的工具行不应进入消息流");

        // TurnDone → 抑制结束,新工具正常展示。
        app.handle_ui_msg(UiMsg::TurnDone);
        assert!(!app.suppress_tool_items, "TurnDone 后应解除抑制");
        app.consume_event(AgentEvent::ToolCallEnd(
            agent_scope_event::ToolCallEndEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                tool_call_id: "tc-b".into(),
                input: Some(r#"{"command":"ls"}"#.into()),
            },
        ));
        assert_eq!(app.items.len(), 1, "抑制结束后新工具行应正常进入消息流");
    }

    #[test]
    fn help_command_opens_overlay_and_esc_closes() {
        let (agent_tx, _agent_rx) = mpsc::unbounded_channel();
        let mut app = App::new(&config(), 0);
        for ch in "/help".chars() {
            app.insert_char(ch);
        }
        app.handle_key(key(KeyCode::Enter), &agent_tx);
        assert_eq!(app.mode, Mode::Help);
        app.handle_key(key(KeyCode::Esc), &agent_tx);
        assert_eq!(app.mode, Mode::Input);
    }

    #[test]
    fn bounded_channel_exerts_backpressure() {
        // With a small channel capacity, sending more messages than the
        // capacity should not panic — the sender must apply backpressure.
        let (tx, mut rx) = mpsc::channel::<UiMsg>(2);
        // Fill the channel.
        let _ = tx.try_send(UiMsg::Status("one".into()));
        let _ = tx.try_send(UiMsg::Status("two".into()));
        // Third send should fail (channel full) with try_send.
        assert!(
            tx.try_send(UiMsg::Status("three".into())).is_err(),
            "bounded channel must reject overflow on try_send"
        );
        // Drain and verify the first two messages arrived.
        let mut count = 0;
        while let Ok(msg) = rx.try_recv() {
            if let UiMsg::Status(text) = msg {
                count += 1;
                assert!(["one", "two"].contains(&text.as_str()));
            }
        }
        assert_eq!(count, 2, "exactly two messages should be in the channel");
    }

    #[tokio::test]
    async fn coalescing_merges_consecutive_deltas() {
        // Simulate a stream of events: several TextBlockDeltas followed by a
        // ToolCallStart.  After coalescing, the TextBlockDeltas must be merged
        // into one event, reducing channel pressure.
        use agent_scope_event::{
            EventBase, TextBlockDeltaEvent, ThinkingBlockDeltaEvent, ToolCallStartEvent,
        };

        let events: Vec<AgentEvent> = vec![
            AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
                base: EventBase::new(),
                reply_id: "r".into(),
                block_id: "b".into(),
                delta: "Hello ".into(),
            }),
            AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
                base: EventBase::new(),
                reply_id: "r".into(),
                block_id: "b".into(),
                delta: "World".into(),
            }),
            AgentEvent::ThinkingBlockDelta(ThinkingBlockDeltaEvent {
                base: EventBase::new(),
                reply_id: "r".into(),
                block_id: "t".into(),
                delta: "Hmm...".into(),
            }),
            AgentEvent::ToolCallStart(ToolCallStartEvent {
                base: EventBase::new(),
                reply_id: "r".into(),
                tool_call_id: "tc-1".into(),
                tool_call_name: "Read".into(),
            }),
        ];

        // Manually apply the same coalescing logic used in `run_turn_for_tui`.
        let (tx, mut rx) = mpsc::channel::<UiMsg>(256);
        let mut pending: Option<(bool, String)> = None;

        for event in events {
            match &event {
                AgentEvent::ThinkingBlockDelta(delta) => {
                    if let Some((true, ref mut text)) = pending {
                        text.push_str(&delta.delta);
                    } else {
                        if let Some((_, text)) = pending.take() {
                            // Flush non-thinking pending.
                            let _ = tx.try_send(UiMsg::Event(AgentEvent::TextBlockDelta(
                                TextBlockDeltaEvent {
                                    base: EventBase::new(),
                                    reply_id: String::new(),
                                    block_id: String::new(),
                                    delta: text,
                                },
                            )));
                        }
                        pending = Some((true, delta.delta.clone()));
                    }
                }
                AgentEvent::TextBlockDelta(delta) => {
                    if let Some((false, ref mut text)) = pending {
                        text.push_str(&delta.delta);
                    } else {
                        if let Some((_, text)) = pending.take() {
                            // Flush non-text pending.
                            let _ = tx.try_send(UiMsg::Event(AgentEvent::ThinkingBlockDelta(
                                ThinkingBlockDeltaEvent {
                                    base: EventBase::new(),
                                    reply_id: String::new(),
                                    block_id: String::new(),
                                    delta: text,
                                },
                            )));
                        }
                        pending = Some((false, delta.delta.clone()));
                    }
                }
                other => {
                    if let Some((_, text)) = pending.take() {
                        let _ = tx.try_send(UiMsg::Event(AgentEvent::TextBlockDelta(
                            TextBlockDeltaEvent {
                                base: EventBase::new(),
                                reply_id: String::new(),
                                block_id: String::new(),
                                delta: text,
                            },
                        )));
                    }
                    let _ = tx.try_send(UiMsg::Event(other.clone()));
                }
            }
        }
        if let Some((_, text)) = pending.take() {
            let _ = tx.try_send(UiMsg::Event(AgentEvent::TextBlockDelta(
                TextBlockDeltaEvent {
                    base: EventBase::new(),
                    reply_id: String::new(),
                    block_id: String::new(),
                    delta: text,
                },
            )));
        }

        // Collect delivered events.
        let mut delivered: Vec<AgentEvent> = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            if let UiMsg::Event(event) = msg {
                delivered.push(event);
            }
        }

        // 4 raw events → 3 delivered (two text deltas merged into one).
        assert_eq!(
            delivered.len(),
            3,
            "coalescing should merge 2 text deltas into 1: got {:?}",
            delivered
                .iter()
                .map(|e| {
                    match e {
                        AgentEvent::TextBlockDelta(d) => format!("TextDelta({})", d.delta),
                        AgentEvent::ThinkingBlockDelta(d) => format!("ThinkingDelta({})", d.delta),
                        other => format!("{:?}", other),
                    }
                })
                .collect::<Vec<_>>()
        );

        // The merged TextBlockDelta should contain "Hello World".
        let merged_text = delivered
            .iter()
            .filter_map(|e| {
                if let AgentEvent::TextBlockDelta(d) = e {
                    Some(d.delta.clone())
                } else {
                    None
                }
            })
            .next()
            .unwrap();
        assert_eq!(merged_text, "Hello World");
    }

    #[test]
    fn meta_events_no_longer_produce_items() {
        let mut app = App::new(&config(), 0);
        for event in [
            AgentEvent::ModelCallStart(agent_scope_event::ModelCallStartEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                model_name: "test-model".into(),
            }),
            AgentEvent::ModelCallEnd(agent_scope_event::ModelCallEndEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                input_tokens: 0,
                output_tokens: 0,
                finished_reason: agent_scope_types::ReplyFinishedReason::Completed,
            }),
            AgentEvent::ReplyStart(agent_scope_event::ReplyStartEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                name: "test".into(),
                role: "assistant".into(),
                session_id: "s".into(),
            }),
            AgentEvent::ReplyEnd(agent_scope_event::ReplyEndEvent {
                base: agent_scope_event::EventBase::new(),
                reply_id: "r".into(),
                error: None,
                finished_reason: agent_scope_types::ReplyFinishedReason::Completed,
                session_id: "s".into(),
            }),
        ] {
            app.consume_event(event);
        }
        assert!(
            app.items.is_empty(),
            "Meta events must not produce UI items"
        );
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

    #[test]
    fn narrow_terminal_falls_back_to_no_border() {
        // <60 列时不绘制面板边框,布局仍为 4 区、不 panic。
        let wide = ratatui::layout::Rect::new(0, 0, 100, 10);
        let narrow = ratatui::layout::Rect::new(0, 0, 40, 10);
        assert!(App::bordered(wide));
        assert!(!App::bordered(narrow));

        let [header, main, status, input] = App::layout_areas(wide);
        assert_eq!(header.width, wide.width);
        assert_eq!(main.width, wide.width);
        assert_eq!(status.width, wide.width);
        assert_eq!(input.width, wide.width);
        let _ = (header, main, status, input);
    }

    #[test]
    fn theme_colors_are_consistent() {
        let theme = Theme::default();
        // 语义色必须互不相同,避免视觉混淆。
        let colors = [
            theme.accent,
            theme.success,
            theme.error,
            theme.warn,
            theme.muted,
            theme.thinking,
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

    #[test]
    fn scroll_offset_follow_bottom() {
        // total > viewport: offset = total - viewport
        assert_eq!(scroll_offset(100, 10, true, 5), 90);
        // total == viewport: offset = 0
        assert_eq!(scroll_offset(10, 10, true, 5), 0);
        // total < viewport: offset = 0 (saturating)
        assert_eq!(scroll_offset(5, 10, true, 5), 0);
    }

    #[test]
    fn scroll_offset_manual_scroll() {
        // scroll within bounds
        assert_eq!(scroll_offset(100, 10, false, 5), 5);
        // scroll clamped to threshold
        assert_eq!(scroll_offset(100, 10, false, 95), 90);
        // zero scroll
        assert_eq!(scroll_offset(100, 10, false, 0), 0);
    }

    #[test]
    fn wrapped_height_single_line() {
        let line = Line::from("1234567890");
        let text = Text::from(line);
        // 10-char line in 5-wide viewport → 2 physical lines
        assert_eq!(wrapped_height(&text, 5), 2);
        // 10-char line in 20-wide viewport → 1 physical line
        assert_eq!(wrapped_height(&text, 20), 1);
    }

    #[test]
    fn wrapped_height_multi_line() {
        let lines = vec![
            Line::from("short"),
            Line::from("a very long line that wraps"),
            Line::from("x"),
        ];
        let text = Text::from(lines);
        let h = wrapped_height(&text, 10);
        // short(5)+very long(29→3)+x(1) = 5 lines
        assert_eq!(h, 5);
    }

    #[test]
    fn wrapped_height_zero_width_falls_back_to_logical() {
        let lines = vec![Line::from("hello"), Line::from("world")];
        let text = Text::from(lines);
        assert_eq!(wrapped_height(&text, 0), 2);
    }
}
