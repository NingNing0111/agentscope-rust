//! AgentScope Foundation Layer — Message & ContentBlock data structures.

#![deny(unsafe_code)]

pub mod block;
pub mod factory;
pub mod msg;
pub mod source;
pub mod state;

// Re-exports — core types
pub use block::{
    BlockType, ContentBlock, DataBlock, DataSource, HintBlock, HintBlockItem, HintContent,
    PermissionRule, TextBlock, ThinkingBlock, ToolCallBlock, ToolOutput, ToolResultBlock,
    ToolResultBlockItem,
};
pub use factory::{
    assistant_msg, assistant_msg_with_blocks, system_msg, system_msg_with_blocks, user_msg,
    user_msg_with_blocks,
};
pub use msg::{AppendEventError, Msg, Role, Usage, ValidationError};
pub use source::{Base64Source, URLSource};
pub use state::{ToolCallState, ToolResultState};
