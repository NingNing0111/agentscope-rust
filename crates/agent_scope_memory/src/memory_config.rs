use crate::MemoryError;

pub const DEFAULT_MEMORY_INSTRUCTIONS: &str = r#"You have access to long-term memory. Use MEMORY.md as an index of saved facts and preferences. When information is relevant, use it to personalize and improve your response. If the index is empty, continue normally and do not mention it unless asked."#;

pub const DEFAULT_RETRIEVAL_INSTRUCTIONS: &str = r#"Select only memory files that are directly relevant to the current user request. Prefer precision over recall. Return an empty selected_files list when no memory is relevant."#;

#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub memory_dir: String,
    pub max_index_tokens: usize,
    pub retrieval_async: bool,
    pub retrieval_max_files: usize,
    pub retrieval_max_tokens_per_file: usize,
    pub retrieval_max_tokens_per_frontmatter: usize,
    pub memory_instructions: String,
    pub retrieval_instructions: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            memory_dir: "Memory".into(),
            max_index_tokens: 4000,
            retrieval_async: true,
            retrieval_max_files: 200,
            retrieval_max_tokens_per_file: 2000,
            retrieval_max_tokens_per_frontmatter: 256,
            memory_instructions: DEFAULT_MEMORY_INSTRUCTIONS.into(),
            retrieval_instructions: DEFAULT_RETRIEVAL_INSTRUCTIONS.into(),
        }
    }
}

impl MemoryConfig {
    pub fn validate(&self) -> Result<(), MemoryError> {
        if self.memory_dir.trim().is_empty() {
            return Err(MemoryError::ValidationError {
                field: "memory_dir".into(),
                message: "memory_dir must not be empty".into(),
            });
        }
        if self.max_index_tokens == 0 {
            return Err(MemoryError::ValidationError {
                field: "max_index_tokens".into(),
                message: "max_index_tokens must be > 0".into(),
            });
        }
        if self.retrieval_max_files == 0 {
            return Err(MemoryError::ValidationError {
                field: "retrieval_max_files".into(),
                message: "retrieval_max_files must be > 0".into(),
            });
        }
        if self.retrieval_max_tokens_per_file == 0 {
            return Err(MemoryError::ValidationError {
                field: "retrieval_max_tokens_per_file".into(),
                message: "retrieval_max_tokens_per_file must be > 0".into(),
            });
        }
        if self.retrieval_max_tokens_per_frontmatter == 0 {
            return Err(MemoryError::ValidationError {
                field: "retrieval_max_tokens_per_frontmatter".into(),
                message: "retrieval_max_tokens_per_frontmatter must be > 0".into(),
            });
        }
        Ok(())
    }
}
