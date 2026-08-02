//! Core [`Memory`] trait — the unified interface for long-term memory backends,
//! covering write, read, delete, list, search, index access, and model-based relevant-memory retrieval.

use std::sync::Arc;

use agent_scope_model::ChatModel;

use crate::{MemoryEntry, MemoryError, MemoryFileHeader, MemoryType};

#[async_trait::async_trait]
pub trait Memory: Send + Sync {
    async fn write(&self, entry: MemoryEntry) -> Result<(), MemoryError>;
    async fn read(&self, name: &str) -> Result<Option<MemoryEntry>, MemoryError>;
    async fn delete(&self, name: &str) -> Result<(), MemoryError>;
    async fn list(&self) -> Result<Vec<MemoryFileHeader>, MemoryError>;
    async fn search(
        &self,
        query: &str,
        type_filter: Option<MemoryType>,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;
    async fn get_index_content(&self) -> Result<String, MemoryError>;
    async fn retrieve_relevant(
        &self,
        query: &str,
        model: &Arc<dyn ChatModel>,
        max_results: usize,
    ) -> Result<Option<String>, MemoryError>;
}
