//! Shared builder helpers for AgentScope examples.
#![allow(dead_code, clippy::type_complexity)]
//!
//! Provides factory functions to create a DashScope-backed chat model, a
//! calculator tool, and a fully configured ReActAgent — keeping each example
//! binary focused on its interaction loop.

use std::sync::Arc;

use agent_scope_agent::{AgentConfig, ContextConfig, MemoryMiddleware, ReActAgent, ReActConfig};
use agent_scope_dashscope::{DashScopeChatModel, DashScopeEmbeddingModel};
use agent_scope_embedding::EmbeddingModelCard;
use agent_scope_memory::{FileMemory, MemoryConfig};
use agent_scope_rag::{
    knowledge_base::KnowledgeBase,
    rag_middleware::{RAGMiddleware, RAGMode},
    vector_store::{DocumentSummary, VectorRecord, VectorSearchResult, VectorStore},
};
use agent_scope_state::{InMemorySessionStore, Session, SessionError, SessionImpl, SessionStore};
use agent_scope_tool::{FunctionTool, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::RwLock;

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

// ---------------------------------------------------------------------------
// Memory agent factory (T001)
// ---------------------------------------------------------------------------

/// Create a ReActAgent configured with MemoryMiddleware backed by FileMemory.
///
/// * `api_key` — DashScope API key.
/// * `model_name` — model id, e.g. `"qwen-plus"`.
/// * `workdir` — temporary directory path for memory storage.
pub fn create_memory_agent(
    api_key: &str,
    model_name: &str,
    workdir: &str,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    let model = create_model(api_key, model_name);

    // Build FileMemory with default config
    let memory_config = MemoryConfig {
        memory_dir: "memory_data".into(),
        ..Default::default()
    };
    let memory: Arc<dyn agent_scope_memory::Memory> =
        Arc::new(FileMemory::new(workdir, memory_config.clone(), None));

    // Wrap in MemoryMiddleware
    let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));

    let system_prompt = concat!(
        "You are a helpful AI assistant with access to long-term memory. ",
        "Use the MEMORY.md index in your system prompt to personalize responses. ",
        "When the user shares a preference or fact, acknowledge it and remember it for future use."
    );

    let builder = AgentConfig::builder()
        .name("memory_agent")
        .system_prompt(system_prompt)
        .model(model);

    let config = builder
        .build()
        .map_err(|e| format!("agent config error: {e}"))?;

    Ok(ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![middleware],
    )?)
}

// ---------------------------------------------------------------------------
// Session helpers (T002)
// ---------------------------------------------------------------------------

/// Create a new InMemorySessionStore.
pub fn create_session_store() -> Arc<InMemorySessionStore> {
    Arc::new(InMemorySessionStore::new())
}

/// Thin wrapper for session test operations with error handling.
pub struct SessionTestHarness {
    pub store: Arc<InMemorySessionStore>,
    pub session: Option<SessionImpl>,
}

impl SessionTestHarness {
    pub fn new(store: Arc<InMemorySessionStore>) -> Self {
        Self {
            store,
            session: None,
        }
    }

    /// Create a new session with the given ID.
    pub fn create_session(&mut self, session_id: &str) -> Result<(), SessionError> {
        let session = SessionImpl::with_session_id(session_id.to_string());
        self.session = Some(session);
        Ok(())
    }

    /// Save the current session to the store.
    pub async fn save_session(&self) -> Result<(), SessionError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| SessionError::NotFound {
                session_id: "no session".into(),
            })?;
        self.store.save(session).await
    }

    /// Load a session from the store by ID.
    pub async fn load_session(&mut self, id: &str) -> Result<(), SessionError> {
        let session = self.store.load(id).await?;
        self.session = Some(session);
        Ok(())
    }

    /// Close the current session.
    pub async fn close_session(&mut self) -> Result<(), SessionError> {
        if let Some(ref mut session) = self.session {
            session.close().await
        } else {
            Err(SessionError::NotFound {
                session_id: "no session".into(),
            })
        }
    }

    /// Delete a session from the store by ID.
    pub async fn delete_session(&self, id: &str) -> Result<(), SessionError> {
        self.store.delete(id).await
    }
}

// ---------------------------------------------------------------------------
// In-memory MockVectorStore (for RAG example, T016)
// ---------------------------------------------------------------------------

/// In-memory mock VectorStore for example/testing purposes.
///
/// Implements the [`VectorStore`] trait with HashMap-backed storage
/// and cosine similarity search.
pub struct MockVectorStore {
    collections: RwLock<HashMap<String, (u32, Vec<VectorRecord>)>>,
}

impl MockVectorStore {
    pub fn new() -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
        }
    }
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

#[async_trait::async_trait]
impl VectorStore for MockVectorStore {
    async fn has_collection(
        &self,
        name: &str,
    ) -> Result<bool, agent_scope_rag::error::VectorStoreError> {
        let guard = self.collections.read().map_err(|e| {
            agent_scope_rag::error::VectorStoreError::BackendError(format!("lock error: {e}"))
        })?;
        Ok(guard.contains_key(name))
    }

    async fn create_collection(
        &self,
        name: &str,
        dimensions: u32,
    ) -> Result<(), agent_scope_rag::error::VectorStoreError> {
        let mut guard = self.collections.write().map_err(|e| {
            agent_scope_rag::error::VectorStoreError::BackendError(format!("lock error: {e}"))
        })?;
        if let Some((existing_dim, _)) = guard.get(name) {
            if *existing_dim != dimensions {
                return Err(
                    agent_scope_rag::error::VectorStoreError::DimensionMismatch {
                        expected: *existing_dim,
                        got: dimensions as usize,
                    },
                );
            }
            return Ok(());
        }
        guard.insert(name.to_string(), (dimensions, vec![]));
        Ok(())
    }

    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<VectorSearchResult>, agent_scope_rag::error::VectorStoreError> {
        let guard = self.collections.read().map_err(|e| {
            agent_scope_rag::error::VectorStoreError::BackendError(format!("lock error: {e}"))
        })?;
        let (_dim, records) = guard.get(collection).ok_or_else(|| {
            agent_scope_rag::error::VectorStoreError::CollectionNotFound(collection.to_string())
        })?;

        let mut scored: Vec<(f32, &VectorRecord)> = records
            .iter()
            .filter(|r| {
                if let Some(ref filter) = metadata_filter {
                    filter
                        .iter()
                        .all(|(k, v)| r.chunk.metadata.get(k) == Some(v))
                } else {
                    true
                }
            })
            .map(|r| {
                let sim = cosine_sim(&query_vector, &r.vector);
                (sim, r)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        if top_k > 0 && top_k < scored.len() {
            scored.truncate(top_k);
        }

        Ok(scored
            .into_iter()
            .map(|(score, record)| VectorSearchResult {
                score,
                document_id: record.document_id.clone(),
                chunk: record.chunk.clone(),
            })
            .collect())
    }

    async fn insert(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), agent_scope_rag::error::VectorStoreError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut guard = self.collections.write().map_err(|e| {
            agent_scope_rag::error::VectorStoreError::BackendError(format!("lock error: {e}"))
        })?;
        let (_dim, existing) = guard.get_mut(collection).ok_or_else(|| {
            agent_scope_rag::error::VectorStoreError::CollectionNotFound(collection.to_string())
        })?;
        existing.extend(records);
        Ok(())
    }

    async fn delete(
        &self,
        collection: &str,
        document_id: &str,
    ) -> Result<(), agent_scope_rag::error::VectorStoreError> {
        let mut guard = self.collections.write().map_err(|e| {
            agent_scope_rag::error::VectorStoreError::BackendError(format!("lock error: {e}"))
        })?;
        if let Some((_dim, records)) = guard.get_mut(collection) {
            records.retain(|r| r.document_id != document_id);
        }
        Ok(())
    }

    async fn list_documents(
        &self,
        collection: &str,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<DocumentSummary>, agent_scope_rag::error::VectorStoreError> {
        let guard = self.collections.read().map_err(|e| {
            agent_scope_rag::error::VectorStoreError::BackendError(format!("lock error: {e}"))
        })?;
        let (_dim, records) = guard.get(collection).ok_or_else(|| {
            agent_scope_rag::error::VectorStoreError::CollectionNotFound(collection.to_string())
        })?;

        let mut summaries: HashMap<String, DocumentSummary> = HashMap::new();
        for record in records {
            if let Some(ref filter) = metadata_filter {
                let matches = filter
                    .iter()
                    .all(|(k, v)| record.chunk.metadata.get(k) == Some(v));
                if !matches {
                    continue;
                }
            }
            let entry = summaries
                .entry(record.document_id.clone())
                .or_insert_with(|| DocumentSummary {
                    document_id: record.document_id.clone(),
                    source: record.chunk.source.clone(),
                    chunk_count: 0,
                    metadata: record.chunk.metadata.clone(),
                });
            entry.chunk_count += 1;
        }
        Ok(summaries.into_values().collect())
    }
}

// ---------------------------------------------------------------------------
// RAG agent factory (T003)
// ---------------------------------------------------------------------------

/// Create a ReActAgent configured with RAGMiddleware backed by a mock
/// vector store and DashScope embedding model.
///
/// * `api_key` — DashScope API key.
/// * `model_name` — chat model id, e.g. `"qwen-plus"`.
/// * `embedding_model_name` — embedding model name, e.g. `"text-embedding-v3"`.
/// * `embedding_dims` — embedding dimensions, e.g. `1536`.
pub fn create_rag_agent(
    api_key: &str,
    model_name: &str,
    embedding_model_name: &str,
    embedding_dims: u32,
) -> Result<(ReActAgent, Arc<KnowledgeBase>, Arc<MockVectorStore>), Box<dyn std::error::Error>> {
    let model = create_model(api_key, model_name);

    // Build embedding model
    let card = EmbeddingModelCard::new(embedding_model_name, embedding_dims, false);
    let embedding_model: Arc<dyn agent_scope_embedding::EmbeddingModel> =
        Arc::new(DashScopeEmbeddingModel::new(api_key.to_string(), card));

    // Build in-memory vector store
    let vector_store: Arc<MockVectorStore> = Arc::new(MockVectorStore::new());
    let vs_for_kb: Arc<dyn VectorStore> = vector_store.clone() as Arc<dyn VectorStore>;

    // Build KnowledgeBase
    let kb = Arc::new(KnowledgeBase::new(
        "test_kb".into(),
        "Test knowledge base for integration testing".into(),
        embedding_model,
        vs_for_kb,
        "test_collection".into(),
        None,
    ));

    // Build RAGMiddleware in Static mode
    let rag_middleware = RAGMiddleware::new(vec![Arc::clone(&kb)], RAGMode::Static, 3, None);

    let system_prompt = concat!(
        "You are a helpful AI assistant with access to a knowledge base. ",
        "Use the injected knowledge to answer questions accurately. ",
        "If no knowledge is provided, respond based on your general knowledge."
    );

    let builder = AgentConfig::builder()
        .name("rag_agent")
        .system_prompt(system_prompt)
        .model(model);

    let config = builder
        .build()
        .map_err(|e| format!("agent config error: {e}"))?;

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![Arc::new(rag_middleware)],
    )?;

    Ok((agent, kb, vector_store))
}

// ---------------------------------------------------------------------------
// TestResult and output utilities (T004)
// ---------------------------------------------------------------------------

/// Standardized result for a single test scenario.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
    pub duration_ms: u64,
}

/// Print a single test result line.
pub fn print_result(result: &TestResult) {
    let icon = if result.passed {
        "\x1b[32m✓\x1b[0m"
    } else {
        "\x1b[31m✗\x1b[0m"
    };
    let duration_s = result.duration_ms as f64 / 1000.0;
    println!("  {icon} {} ({duration_s:.1}s)", result.name,);
    if !result.detail.is_empty() {
        println!("    {}", result.detail);
    }
}

/// Print a section header for a test.
pub fn print_test_header(n: usize, name: &str) {
    println!("── {}. {} ──", n, name);
}

/// Print a summary of all test results.
pub fn print_summary(results: &[TestResult], total_start: std::time::Instant) {
    let total_ms = total_start.elapsed().as_millis() as u64;
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.len() - passed;
    println!();
    if failed == 0 {
        println!(
            "\x1b[32mALL {} TESTS PASSED\x1b[0m ({:.1}s total)",
            results.len(),
            total_ms as f64 / 1000.0,
        );
    } else {
        println!(
            "\x1b[31m{} passed, {} FAILED\x1b[0m ({:.1}s total)",
            passed,
            failed,
            total_ms as f64 / 1000.0,
        );
    }
}

/// Print an example header banner.
pub fn print_banner(name: &str, model: &str) {
    println!("╔══════════════════════════════════════════════╗");
    println!("║   AgentScope {} Test       ║", name);
    println!("║   Model: {:<36}║", model);
    println!("╚══════════════════════════════════════════════╝");
    println!();
}
