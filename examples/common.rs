//! Shared builder helpers for AgentScope examples.
//!
//! Provides factory functions to create a DashScope-backed chat model, a
//! calculator tool, and a fully configured ReActAgent — keeping each example
//! binary focused on its interaction loop.

use std::sync::Arc;

use agent_scope_agent::{AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_tool::{FunctionTool, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Model creation
// ---------------------------------------------------------------------------

/// Create a DashScope chat model connected to Alibaba Cloud Model Studio.
///
/// * `api_key` — DashScope API key (starts with `sk-`).
/// * `model_name` — model id, e.g. `"qwen-plus"`, `"qwen-max"`.
pub fn create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel> {
    Arc::new(DashScopeChatModel::new(api_key, model_name))
}

/// Create a DashScope chat model with thinking/reasoning mode enabled.
///
/// When `thinking_budget` is `Some(n)`, the model will allocate up to `n` tokens
/// for internal reasoning (returned as `ThinkingBlock` deltas via streaming).
/// When `None`, thinking is enabled without a hard budget.
pub fn create_model_with_thinking(
    api_key: &str,
    model_name: &str,
    thinking_budget: Option<u32>,
) -> Arc<DashScopeChatModel> {
    let mut model = DashScopeChatModel::new(api_key, model_name);
    model.parameters.enable_thinking = true;
    model.parameters.thinking_budget = thinking_budget;
    model.stream = true;
    Arc::new(model)
}

// ---------------------------------------------------------------------------
// Calculator tool
// ---------------------------------------------------------------------------

/// Input schema for the calculator tool — auto-derived via schemars.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    /// A mathematical expression like "1 + 2 * 3 / (4 - 1) ^ 2"
    expression: String,
}

/// Simple recursive-descent expression evaluator.
///
/// Supports: `+`, `-`, `*`, `/`, `^` (power, right-associative), `()`,
/// and unary `-` / `+`.  All values are `f64`.
///
/// Grammar:
/// ```text
/// expr    → term (('+' | '-') term)*
/// term    → factor (('*' | '/') factor)*
/// factor  → unary ('^' factor)?
/// unary   → ('+' | '-')? atom
/// atom    → '(' expr ')' | NUMBER
/// ```
mod calc {
    use std::f64::consts;

    #[derive(Debug, Clone)]
    pub struct ParseError {
        pub reason: String,
    }

    impl std::fmt::Display for ParseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.reason)
        }
    }

    struct Parser {
        chars: Vec<char>,
        pos: usize,
    }

    impl Parser {
        fn new(input: &str) -> Self {
            let chars: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
            Self { chars, pos: 0 }
        }

        fn peek(&self) -> Option<char> {
            self.chars.get(self.pos).copied()
        }

        fn advance(&mut self) -> Option<char> {
            let c = self.peek();
            if c.is_some() {
                self.pos += 1;
            }
            c
        }

        fn expect_digit(&mut self) -> Result<(), ParseError> {
            match self.peek() {
                Some(c) if c.is_ascii_digit() || c == '.' => Ok(()),
                Some(c) => Err(ParseError {
                    reason: format!("expected digit, found '{c}'"),
                }),
                None => Err(ParseError {
                    reason: "unexpected end of expression".into(),
                }),
            }
        }

        fn number(&mut self) -> Result<f64, ParseError> {
            self.expect_digit()?;
            let start = self.pos;
            while let Some(c) = self.peek()
                && (c.is_ascii_digit() || c == '.')
            {
                self.advance();
            }
            let num_str: String = self.chars[start..self.pos].iter().collect();
            num_str.parse::<f64>().map_err(|e| ParseError {
                reason: format!("invalid number '{num_str}': {e}"),
            })
        }

        // Handle built-in constants
        fn atom(&mut self) -> Result<f64, ParseError> {
            match self.peek() {
                Some('(') => {
                    self.advance(); // '('
                    let val = self.expr()?;
                    match self.peek() {
                        Some(')') => {
                            self.advance(); // ')'
                            Ok(val)
                        }
                        Some(c) => Err(ParseError {
                            reason: format!("expected ')', found '{c}'"),
                        }),
                        None => Err(ParseError {
                            reason: "unclosed parenthesis".into(),
                        }),
                    }
                }
                Some('p') | Some('P') => {
                    // check for "pi" (case-insensitive)
                    let remaining: String = self.chars[self.pos..].iter().collect();
                    let lower = remaining.to_lowercase();
                    if lower.starts_with("pi") {
                        // advance past "pi" (2 chars)
                        let skip = 2.min(self.chars.len() - self.pos);
                        for _ in 0..skip {
                            self.advance();
                        }
                        Ok(consts::PI)
                    } else {
                        Err(ParseError {
                            reason: format!("unexpected characters: {remaining}"),
                        })
                    }
                }
                Some('e') => {
                    self.advance();
                    // If followed by digits, it's scientific notation in the number parser
                    // Otherwise it's Euler's number
                    if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        // It's actually part of a number like e5 — backtrack
                        self.pos -= 1;
                        self.number()
                    } else {
                        Ok(consts::E)
                    }
                }
                Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
                Some(c) => Err(ParseError {
                    reason: format!("unexpected character '{c}'"),
                }),
                None => Err(ParseError {
                    reason: "unexpected end of expression".into(),
                }),
            }
        }

        fn unary(&mut self) -> Result<f64, ParseError> {
            match self.peek() {
                Some('+') => {
                    self.advance();
                    self.unary()
                }
                Some('-') => {
                    self.advance();
                    Ok(-self.unary()?)
                }
                _ => self.atom(),
            }
        }

        fn factor(&mut self) -> Result<f64, ParseError> {
            let base = self.unary()?;
            match self.peek() {
                Some('^') => {
                    self.advance();
                    let exponent = self.factor()?; // right-associative
                    Ok(base.powf(exponent))
                }
                _ => Ok(base),
            }
        }

        fn term(&mut self) -> Result<f64, ParseError> {
            let mut left = self.factor()?;
            loop {
                match self.peek() {
                    Some('*') => {
                        self.advance();
                        left *= self.factor()?;
                    }
                    Some('/') => {
                        self.advance();
                        let rhs = self.factor()?;
                        if rhs == 0.0 {
                            return Err(ParseError {
                                reason: "division by zero".into(),
                            });
                        }
                        left /= rhs;
                    }
                    _ => break,
                }
            }
            Ok(left)
        }

        fn expr(&mut self) -> Result<f64, ParseError> {
            let mut left = self.term()?;
            loop {
                match self.peek() {
                    Some('+') => {
                        self.advance();
                        left += self.term()?;
                    }
                    Some('-') => {
                        self.advance();
                        left -= self.term()?;
                    }
                    _ => break,
                }
            }
            Ok(left)
        }

        pub fn parse(&mut self) -> Result<f64, ParseError> {
            let value = self.expr()?;
            if self.peek().is_some() {
                return Err(ParseError {
                    reason: format!(
                        "unexpected trailing characters starting at position {}",
                        self.pos
                    ),
                });
            }
            Ok(value)
        }
    }

    pub fn evaluate(expr: &str) -> Result<f64, ParseError> {
        Parser::new(expr).parse()
    }
}

/// Evaluate expression and return a human-friendly result.
async fn calc_handler(input: CalcInput) -> String {
    match calc::evaluate(&input.expression) {
        Ok(value) => {
            // Round to at most 6 fractional digits, strip trailing zeros
            let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
            format!(
                "{input} = {output}",
                input = input.expression,
                output = rounded
            )
        }
        Err(e) => format!(
            "Error evaluating '{input}': {err}",
            input = input.expression,
            err = e
        ),
    }
}

/// Create a calculator tool that evaluates mathematical expressions.
///
/// Supports `+`, `-`, `*`, `/`, `^` (power), parentheses, and the constants
/// `pi` and `e`.
pub fn create_calculator_tool() -> FunctionTool {
    FunctionTool::new(
        "calculator",
        "Evaluate a mathematical expression. Supports +, -, *, /, ^ (power), (), and constants pi/e. Example: \"2 + 3 * (4 - 1) ^ 2\"",
        calc_handler,
    )
}

// ---------------------------------------------------------------------------
// Agent builder
// ---------------------------------------------------------------------------

/// Build a ready-to-use ReActAgent with the given model and tools.
///
/// The agent is configured with a helpful system prompt that encourages it to
/// use tools when appropriate.
pub fn build_agent(
    model: Arc<DashScopeChatModel>,
    toolkit: Option<ToolKit>,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    let system_prompt = concat!(
        "You are a helpful AI assistant. ",
        "When the user asks a mathematical question, use the 'calculator' tool to compute the answer. ",
        "Always show your work: state what expression you are evaluating, call the tool, ",
        "then explain the result in plain language."
    );

    let mut builder = AgentConfig::builder()
        .name("assistant")
        .system_prompt(system_prompt)
        .model(model);

    if let Some(tk) = toolkit {
        builder = builder.toolkit(tk);
    }

    let config = builder
        .build()
        .map_err(|e| format!("agent config error: {e}"))?;

    Ok(ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?)
}
