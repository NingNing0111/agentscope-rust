use std::io::{self, Write};

use agent_scope_event::AgentEvent;

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub show_events: bool,
    pub show_json_events: bool,
    pub secrets: Vec<String>,
}

pub struct Renderer {
    options: RenderOptions,
    text_started: bool,
    thinking_started: bool,
}

impl Renderer {
    pub fn new(options: RenderOptions) -> Self {
        Self {
            options,
            text_started: false,
            thinking_started: false,
        }
    }

    pub fn render(&mut self, event: &AgentEvent) -> io::Result<()> {
        if self.options.show_json_events {
            let json = serde_json::to_string(event).unwrap_or_else(|err| {
                format!(r#"{{"type":"SERIALIZATION_ERROR","error":"{err}"}}"#)
            });
            eprintln!("[event:json] {}", self.mask(&json));
        }

        match event {
            AgentEvent::ReplyStart(e) => self.lifecycle(format_args!(
                "[reply:start] name={} role={} reply_id={}",
                e.name, e.role, e.reply_id
            )),
            AgentEvent::ReplyEnd(e) => {
                if self.text_started {
                    println!();
                    self.text_started = false;
                }
                self.lifecycle(format_args!(
                    "[reply:end] reason={:?} error={}",
                    e.finished_reason,
                    e.error
                        .as_ref()
                        .map(|err| self.mask(&format!("{err:?}")))
                        .unwrap_or_else(|| "none".to_string())
                ))
            }
            AgentEvent::ModelCallStart(e) => self.lifecycle(format_args!(
                "[model:start] model={} reply_id={}",
                e.model_name, e.reply_id
            )),
            AgentEvent::ModelCallEnd(e) => self.lifecycle(format_args!(
                "[model:end] reason={:?} input_tokens={} output_tokens={}",
                e.finished_reason, e.input_tokens, e.output_tokens
            )),
            AgentEvent::TextBlockStart(_) => {
                if !self.text_started {
                    self.text_started = true;
                }
                Ok(())
            }
            AgentEvent::TextBlockDelta(e) => {
                print!("{}", self.mask(&e.delta));
                io::stdout().flush()
            }
            AgentEvent::TextBlockEnd(e) => {
                if self.options.show_events {
                    if let Some(text) = &e.text {
                        eprintln!("[text:end] chars={}", text.chars().count());
                    } else {
                        eprintln!("[text:end]");
                    }
                }
                Ok(())
            }
            AgentEvent::ThinkingBlockStart(_) => {
                self.thinking_started = true;
                if self.options.show_events {
                    eprintln!("[thinking:start]");
                }
                Ok(())
            }
            AgentEvent::ThinkingBlockDelta(e) => {
                if self.options.show_events {
                    eprintln!("[thinking:delta] {}", self.mask(&e.delta));
                }
                Ok(())
            }
            AgentEvent::ThinkingBlockEnd(e) => {
                if self.options.show_events || self.thinking_started {
                    let chars = e.thinking.as_ref().map(|value| value.chars().count());
                    if let Some(chars) = chars {
                        eprintln!("[thinking:end] chars={chars}");
                    } else if self.options.show_events {
                        eprintln!("[thinking:end]");
                    }
                }
                self.thinking_started = false;
                Ok(())
            }
            AgentEvent::ToolCallStart(e) => {
                self.ensure_text_newline();
                eprintln!("[tool:start] {} ({})", e.tool_call_name, e.tool_call_id);
                Ok(())
            }
            AgentEvent::ToolCallDelta(e) => {
                if self.options.show_events {
                    eprintln!("[tool:args] {}", self.mask(&e.delta));
                }
                Ok(())
            }
            AgentEvent::ToolCallEnd(e) => {
                if self.options.show_events {
                    let input = e.input.as_deref().map(|value| self.mask(value));
                    eprintln!(
                        "[tool:ready] {} input={}",
                        e.tool_call_id,
                        input.unwrap_or_else(|| "<unknown>".to_string())
                    );
                }
                Ok(())
            }
            AgentEvent::ToolResultStart(e) => {
                eprintln!(
                    "[tool:result:start] {} ({})",
                    e.tool_call_name, e.tool_call_id
                );
                Ok(())
            }
            AgentEvent::ToolResultTextDelta(e) => {
                eprintln!("[tool:result] {}", self.mask(&e.delta));
                Ok(())
            }
            AgentEvent::ToolResultDataDelta(e) => {
                if self.options.show_events {
                    eprintln!(
                        "[tool:result:data] media_type={} block_id={}",
                        e.media_type, e.block_id
                    );
                }
                Ok(())
            }
            AgentEvent::ToolResultEnd(e) => {
                eprintln!("[tool:result:end] state={:?}", e.state);
                if self.options.show_events
                    && let Some(output) = &e.output
                {
                    eprintln!("[tool:result:output] {}", self.mask(output));
                }
                Ok(())
            }
            AgentEvent::RequireUserConfirm(e) => {
                self.ensure_text_newline();
                eprintln!(
                    "[permission:confirm-required] {} tool call(s) require confirmation; this demo relies on the framework decision path.",
                    e.tool_calls.len()
                );
                Ok(())
            }
            AgentEvent::UserInterrupt(e) => {
                self.lifecycle(format_args!("[reply:interrupted] reply_id={}", e.reply_id))
            }
            AgentEvent::ExceedMaxIters(e) => self.lifecycle(format_args!(
                "[reply:max-iters] name={} reply_id={}",
                e.name, e.reply_id
            )),
            AgentEvent::DataBlockStart(e) => self.lifecycle(format_args!(
                "[data:start] media_type={} block_id={}",
                e.media_type, e.block_id
            )),
            AgentEvent::DataBlockDelta(e) => self.lifecycle(format_args!(
                "[data:delta] media_type={} bytes={}",
                e.media_type,
                e.data.len()
            )),
            AgentEvent::DataBlockEnd(e) => {
                self.lifecycle(format_args!("[data:end] block_id={}", e.block_id))
            }
            AgentEvent::HintBlock(e) => self.lifecycle(format_args!(
                "[hint] block_id={} source={}",
                e.block_id,
                e.source.as_deref().unwrap_or("unknown")
            )),
            AgentEvent::UserConfirmResult(_)
            | AgentEvent::RequireExternalExecution(_)
            | AgentEvent::ExternalExecutionResult(_)
            | AgentEvent::Custom(_)
            | AgentEvent::SessionCreated(_)
            | AgentEvent::SessionClosed(_)
            | AgentEvent::SessionSaved(_)
            | AgentEvent::SessionLoaded(_)
            | AgentEvent::SessionTrimmed(_) => {
                if self.options.show_events {
                    eprintln!("[event] {}", self.mask(format_event_name(event)));
                }
                Ok(())
            }
        }
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if self.text_started {
            println!();
            self.text_started = false;
        }
        io::stdout().flush()
    }

    fn lifecycle(&self, args: std::fmt::Arguments<'_>) -> io::Result<()> {
        if self.options.show_events {
            eprintln!("{args}");
        }
        Ok(())
    }

    fn ensure_text_newline(&mut self) {
        if self.text_started {
            eprintln!();
            self.text_started = false;
        }
    }

    fn mask(&self, text: &str) -> String {
        mask_text(text, &self.options.secrets)
    }
}

pub fn mask_text(text: &str, secrets: &[String]) -> String {
    let mut masked = text.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            let replacement = mask_secret(secret);
            masked = masked.replace(secret, &replacement);
        }
    }

    mask_sk_like_values(&masked)
}

fn mask_secret(secret: &str) -> String {
    let suffix: String = secret
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if suffix.is_empty() {
        "[REDACTED]".to_string()
    } else {
        format!("[REDACTED:{suffix}]")
    }
}

fn mask_sk_like_values(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut index = 0;

    while let Some(relative_start) = text[index..].find("sk-") {
        let start = index + relative_start;
        output.push_str(&text[index..start]);

        let mut end = start;
        for ch in text[start..].chars() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                end += ch.len_utf8();
            } else {
                break;
            }
        }

        if end - start > 8 {
            output.push_str("sk-[REDACTED]");
        } else {
            output.push_str(&text[start..end]);
        }
        index = end;
    }

    output.push_str(&text[index..]);
    output
}

fn format_event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::ReplyStart(_) => "REPLY_START",
        AgentEvent::ReplyEnd(_) => "REPLY_END",
        AgentEvent::ModelCallStart(_) => "MODEL_CALL_START",
        AgentEvent::ModelCallEnd(_) => "MODEL_CALL_END",
        AgentEvent::TextBlockStart(_) => "TEXT_BLOCK_START",
        AgentEvent::TextBlockDelta(_) => "TEXT_BLOCK_DELTA",
        AgentEvent::TextBlockEnd(_) => "TEXT_BLOCK_END",
        AgentEvent::DataBlockStart(_) => "DATA_BLOCK_START",
        AgentEvent::DataBlockDelta(_) => "DATA_BLOCK_DELTA",
        AgentEvent::DataBlockEnd(_) => "DATA_BLOCK_END",
        AgentEvent::ThinkingBlockStart(_) => "THINKING_BLOCK_START",
        AgentEvent::ThinkingBlockDelta(_) => "THINKING_BLOCK_DELTA",
        AgentEvent::ThinkingBlockEnd(_) => "THINKING_BLOCK_END",
        AgentEvent::HintBlock(_) => "HINT_BLOCK",
        AgentEvent::ToolCallStart(_) => "TOOL_CALL_START",
        AgentEvent::ToolCallDelta(_) => "TOOL_CALL_DELTA",
        AgentEvent::ToolCallEnd(_) => "TOOL_CALL_END",
        AgentEvent::ToolResultStart(_) => "TOOL_RESULT_START",
        AgentEvent::ToolResultTextDelta(_) => "TOOL_RESULT_TEXT_DELTA",
        AgentEvent::ToolResultDataDelta(_) => "TOOL_RESULT_DATA_DELTA",
        AgentEvent::ToolResultEnd(_) => "TOOL_RESULT_END",
        AgentEvent::ExceedMaxIters(_) => "EXCEED_MAX_ITERS",
        AgentEvent::RequireUserConfirm(_) => "REQUIRE_USER_CONFIRM",
        AgentEvent::UserConfirmResult(_) => "USER_CONFIRM_RESULT",
        AgentEvent::UserInterrupt(_) => "USER_INTERRUPT",
        AgentEvent::RequireExternalExecution(_) => "REQUIRE_EXTERNAL_EXECUTION",
        AgentEvent::ExternalExecutionResult(_) => "EXTERNAL_EXECUTION_RESULT",
        AgentEvent::Custom(_) => "CUSTOM",
        AgentEvent::SessionCreated(_) => "SESSION_CREATED",
        AgentEvent::SessionClosed(_) => "SESSION_CLOSED",
        AgentEvent::SessionSaved(_) => "SESSION_SAVED",
        AgentEvent::SessionLoaded(_) => "SESSION_LOADED",
        AgentEvent::SessionTrimmed(_) => "SESSION_TRIMMED",
    }
}
