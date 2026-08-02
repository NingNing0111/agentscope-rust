//! Core memory data models — [`MemoryEntry`], [`MemoryMetadata`], [`MemoryType`],
//! and [`MemoryFileHeader`] types used across the memory system.

use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
    #[serde(untagged)]
    Unknown(String),
}

impl MemoryType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl From<&str> for MemoryType {
    fn from(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "user" => Self::User,
            "feedback" => Self::Feedback,
            "project" => Self::Project,
            "reference" => Self::Reference,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    #[serde(rename = "type")]
    pub mem_type: MemoryType,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl MemoryMetadata {
    pub fn new(mem_type: MemoryType) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            mem_type,
            created_at: now.clone(),
            updated_at: now,
            tags: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub name: String,
    pub description: String,
    pub metadata: MemoryMetadata,
    pub content: String,
}

impl MemoryEntry {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        mem_type: MemoryType,
        content: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            metadata: MemoryMetadata::new(mem_type),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryFileHeader {
    pub filename: String,
    pub path: String,
    pub description: Option<String>,
    pub mem_type: Option<MemoryType>,
    pub mtime: Option<f64>,
}
