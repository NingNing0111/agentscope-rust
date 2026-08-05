//! ratatui TUI frontend for the pi-rust coding Agent.
//!
//! The agent runs in a dedicated tokio task and streams render events and
//! confirmation requests to the UI over an mpsc channel; the UI event loop
//! alternates between keyboard events (`crossterm::event::EventStream`) and
//! channel messages, redrawing the screen after every processed event. This
//! lets thinking blocks, assistant text and tool calls render incrementally
//! as the model streams them.

#![deny(unsafe_code)]

use std::collections::HashMap;
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

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};

use crate::agent::AgentRuntime;
use crate::config::RuntimeConfig;
use crate::error::{PiError, PiResult};
use crate::render::{
    ConfirmationCandidate, RenderConfig, RenderedTurn, event_name, render_event, tool_call_summary,
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
    ToolCall { name: String, summary: String },
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

pub struct App {
    input: String,
    cursor: usize,
    items: Vec<UiItem>,
    scroll: u16,
    follow_bottom: bool,
    mode: Mode,
    confirm: Option<ConfirmUi>,
    busy: bool,
    status: String,
    help_text: String,
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
            busy: false,
            status: String::new(),
            help_text,
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
            AgentEvent::ToolCallEnd(end) => {
                let name = self
                    .tool_call_names
                    .get(&end.tool_call_id)
                    .cloned()
                    .unwrap_or_else(|| "?".to_string());
                let summary = tool_call_summary(&name, end.input.as_deref());
                self.items.push(UiItem::ToolCall { name, summary });
                self.follow_bottom = true;
            }
            AgentEvent::ToolResultTextDelta(delta) => {
                let entry = self
                    .tool_outputs
                    .entry(delta.tool_call_id.clone())
                    .or_default();
                if entry.chars().count() < TOOL_OUTPUT_CAP {
                    entry.push_str(&delta.delta);
                }
            }
            AgentEvent::ToolResultEnd(end) => {
                let line = tool_result_line(&self.tool_outputs, end);
                self.items.push(UiItem::ToolResult(line));
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
                self.items
                    .push(UiItem::Meta(event_name(&event).to_string()));
                self.follow_bottom = true;
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

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let [header, main, status, input] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        self.render_header(frame, header);
        self.render_message_area(frame, main);
        self.render_status(frame, status);
        self.render_input(frame, input);

        match self.mode {
            Mode::Confirm => self.render_overlay(frame, "confirm", self.confirm_lines()),
            Mode::Help => self.render_overlay(frame, "help", self.help_lines()),
            Mode::Input => {}
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let busy_style = if self.busy {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Green)
        };
        let state = if self.busy { " running " } else { " idle " };
        let line = Line::from(vec![
            Span::styled(
                " pi-rust ",
                Style::default().fg(Color::Black).bg(Color::Cyan),
            ),
            Span::raw(format!(
                " {} · {} · mode {} · cwd {} · skills {} ",
                self.provider, self.model, self.mode_name, self.cwd, self.skills
            )),
            Span::styled(state, busy_style),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_message_area(&mut self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        let lines = self.items_to_lines();
        let total = lines.len();
        let offset = if self.follow_bottom {
            total.saturating_sub(area.height as usize)
        } else {
            (self.scroll as usize).min(total.saturating_sub(area.height as usize))
        };
        self.scroll = offset as u16;
        let paragraph = Paragraph::new(Text::from(lines)).scroll((offset as u16, 0));
        frame.render_widget(paragraph, area);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let text = format!("  {}", self.status);
        let style = if self.busy {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        frame.render_widget(Paragraph::new(text).style(style), area);
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
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
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
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
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                }
                UiItem::UserMsg(text) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "user ",
                            Style::default()
                                .fg(Color::Green)
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
                                Span::styled("⋯ ", Style::default().fg(Color::DarkGray)),
                                Span::styled(
                                    sub.to_string(),
                                    Style::default()
                                        .fg(Color::DarkGray)
                                        .add_modifier(Modifier::ITALIC),
                                ),
                            ]));
                        }
                    }
                }
                UiItem::ToolCall { name, summary } => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{name} "),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(summary.clone(), Style::default().fg(Color::Cyan)),
                    ]));
                }
                UiItem::ToolResult(text) => {
                    let color = if text.starts_with("→ success") {
                        Color::Green
                    } else {
                        Color::Red
                    };
                    lines.push(Line::from(Span::styled(
                        text.clone(),
                        Style::default().fg(color),
                    )));
                }
                UiItem::Meta(text) => {
                    lines.push(Line::from(Span::styled(
                        format!("· {text}"),
                        Style::default().fg(Color::DarkGray),
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
    let (ui_tx, mut ui_rx) = mpsc::unbounded_channel::<UiMsg>();
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
    ui_tx: mpsc::UnboundedSender<UiMsg>,
    mut agent_rx: mpsc::UnboundedReceiver<AgentCmd>,
) {
    while let Some(cmd) = agent_rx.recv().await {
        match cmd {
            AgentCmd::Prompt(input) => {
                let _ = ui_tx.send(UiMsg::SetBusy(true));
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
                            let _ = ui_tx.send(UiMsg::Status(format!(
                                "save failed: {}",
                                err.safe_message()
                            )));
                        }
                    }
                    Err(err) => {
                        let _ = ui_tx.send(UiMsg::Status(format!("error: {}", err.safe_message())));
                    }
                }
                let _ = ui_tx.send(UiMsg::SetBusy(false));
                let _ = ui_tx.send(UiMsg::TurnDone);
            }
            AgentCmd::Command(command) => {
                let output = handle_command(&mut runtime, &command);
                match output {
                    Ok(output) => {
                        let should_exit = output.should_exit;
                        let _ = ui_tx.send(UiMsg::CommandOutput(output));
                        if should_exit {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = ui_tx.send(UiMsg::Status(format!("error: {}", err.safe_message())));
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
async fn run_turn_for_tui(
    runtime: &AgentRuntime,
    input: &str,
    ui_tx: &mpsc::UnboundedSender<UiMsg>,
) -> PiResult<RenderedTurn> {
    let msg = user_msg("user", input)?;
    let mut stream = runtime.agent.reply_stream(Some(vec![msg])).await?;
    let config = RenderConfig {
        cwd: runtime.config.cwd.clone(),
        show_events: false,
        show_json_events: false,
    };
    let mut turn = RenderedTurn::default();
    while let Some(event) = stream.next().await {
        render_event(event.clone(), &config, &mut turn)?;
        let _ = ui_tx.send(UiMsg::Event(event));
    }
    Ok(turn)
}

/// `repl::run_turn_with_confirmations` variant whose ask closure hands the
/// candidate list to the UI and waits for the keypress decisions.
async fn run_turn_with_confirmations_tui(
    runtime: &AgentRuntime,
    input: &str,
    ui_tx: &mpsc::UnboundedSender<UiMsg>,
) -> PiResult<RenderedTurn> {
    let approvals = std::sync::Arc::clone(&runtime.approvals);
    let first = run_turn_for_tui(runtime, input, ui_tx).await?;
    let ask = |candidates: &[ConfirmationCandidate]| {
        let ui_tx = ui_tx.clone();
        let candidates: Vec<ConfirmationCandidate> = candidates.to_vec();
        async move {
            let (reply_tx, reply_rx) = oneshot::channel();
            let _ = ui_tx.send(UiMsg::ConfirmRequest {
                candidates,
                reply: reply_tx,
            });
            reply_rx.await.unwrap_or_default()
        }
    };
    let result = run_confirmation_loop(
        &approvals,
        first,
        || async {
            run_turn_for_tui(runtime, input, ui_tx)
                .await
                .unwrap_or_default()
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
            app.items[0],
            UiItem::ToolCall { ref name, ref summary } if name == "Bash" && summary.contains("ls")
        ));
        assert_eq!(app.items[1], UiItem::ToolResult("→ success".to_string()));
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
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm a".into(),
                    description: "[Bash] $ rm a".into(),
                },
                ConfirmationCandidate {
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
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm a".into(),
                    description: "[Bash] $ rm a".into(),
                },
                ConfirmationCandidate {
                    tool_name: "Bash".into(),
                    fingerprint: "bash:rm b".into(),
                    description: "[Bash] $ rm b".into(),
                },
                ConfirmationCandidate {
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
}
