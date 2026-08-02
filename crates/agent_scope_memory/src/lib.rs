//! AgentScope Memory System — persistent long-term memory storage and retrieval.

#![deny(unsafe_code)]

pub mod backend;
pub mod file_memory;
pub mod frontmatter;
pub mod index;
pub mod memory_config;
pub mod memory_entry;
pub mod memory_error;
pub mod memory_trait;
pub mod retrieval;
pub mod turbovec_memory;

pub use backend::{Backend, LocalBackend};
pub use file_memory::FileMemory;
pub use frontmatter::{parse_frontmatter_fields, serialize_frontmatter};
pub use index::{read_index, remove_index_line, truncate_index, write_index_line};
pub use memory_config::{
    DEFAULT_MEMORY_INSTRUCTIONS, DEFAULT_RETRIEVAL_INSTRUCTIONS, MemoryConfig,
};
pub use memory_entry::{MemoryEntry, MemoryFileHeader, MemoryMetadata, MemoryType};
pub use memory_error::MemoryError;
pub use memory_trait::Memory;
pub use retrieval::MemorySelection;
pub use turbovec_memory::{
    MemoryRebuildReport, MemorySearchResult, MemoryVectorHit, MemoryVectorIndex,
    MemoryVectorRecord, TurbovecMemory, TurbovecMemoryConfig, VectorIndexStatus,
};
