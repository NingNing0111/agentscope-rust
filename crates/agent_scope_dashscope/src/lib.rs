//! AgentScope DashScope Provider — Qwen/Model Studio models via OpenAI-compatible API.

#![deny(unsafe_code)]

pub mod embedding;
pub mod formatter;
pub mod model;
pub mod parameters;

pub use embedding::DashScopeEmbeddingModel;
pub use formatter::DashScopeFormatter;
pub use model::DashScopeChatModel;
pub use parameters::DashScopeParameters;
