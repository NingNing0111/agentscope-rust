//! AgentScope Model API — ChatModel trait, ChatResponse, Formatter, ModelCard,
//! and provider-independent abstractions.

#![deny(unsafe_code)]

pub mod accumulator;
pub mod card;
pub mod formatter;
pub mod json_repair;
pub mod model_error;
pub mod model_trait;
pub mod response;
pub mod schema_flat;
pub mod tool_choice;
pub mod usage;
pub mod wav_header;

// Re-exports — core types
pub use accumulator::StreamAccumulator;
pub use card::{ModelCard, ModelStatus};
pub use formatter::{FormatError, Formatter, MessageGroup};
pub use model_error::{ModelError, ModelErrorKind};
pub use model_trait::{ChatModel, ModelCallResult};
pub use response::{ChatResponse, FinishedReason, StructuredResponse};
pub use tool_choice::ToolChoice;
pub use usage::ChatUsage;
