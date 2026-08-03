use std::fmt;

use thiserror::Error;

pub type PiResult<T> = Result<T, PiError>;

#[derive(Debug, Error)]
pub enum PiError {
    #[error("invalid configuration for {field}: {message}")]
    Config {
        field: &'static str,
        message: String,
    },
    #[error("I/O error during {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("session error: {message}")]
    Session { message: String },
    #[error("tool error: {message}")]
    Tool { message: String },
    #[error("model error: {message}")]
    Model { message: String },
    #[error("unsupported feature: {message}")]
    Unsupported { message: String },
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl PiError {
    pub fn config(field: &'static str, message: impl Into<String>) -> Self {
        Self::Config {
            field,
            message: message.into(),
        }
    }

    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }

    pub fn session(message: impl Into<String>) -> Self {
        Self::Session {
            message: message.into(),
        }
    }

    pub fn tool(message: impl Into<String>) -> Self {
        Self::Tool {
            message: message.into(),
        }
    }

    pub fn model(message: impl Into<String>) -> Self {
        Self::Model {
            message: message.into(),
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported {
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Config { .. } => 2,
            _ => 1,
        }
    }

    pub fn safe_message(&self) -> String {
        redact_secrets(&self.to_string())
    }
}

impl From<agent_scope_agent::AgentError> for PiError {
    fn from(value: agent_scope_agent::AgentError) -> Self {
        Self::model(value.to_string())
    }
}

impl From<agent_scope_message::ValidationError> for PiError {
    fn from(value: agent_scope_message::ValidationError) -> Self {
        Self::internal(format!("message validation failed: {value:?}"))
    }
}

impl From<agent_scope_rag::VectorStoreError> for PiError {
    fn from(value: agent_scope_rag::VectorStoreError) -> Self {
        Self::internal(format!("RAG vector store setup failed: {value}"))
    }
}

impl From<serde_json::Error> for PiError {
    fn from(value: serde_json::Error) -> Self {
        Self::session(value.to_string())
    }
}

pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();
    for key in ["API_KEY", "DASHSCOPE_API_KEY", "api_key", "authorization"] {
        out = out.replace(key, "[redacted-key-name]");
    }
    out
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Validation,
    Model,
    Tool,
    Permission,
    Io,
    Session,
    Internal,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self)
                .unwrap_or_else(|_| "internal".into())
                .trim_matches('"')
        )
    }
}
