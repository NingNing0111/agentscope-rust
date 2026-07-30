use std::sync::Arc;

use regex::Regex;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::frontmatter::{body_after_frontmatter, parse_frontmatter_fields, serialize_frontmatter};
use crate::index;
use crate::{
    Backend, LocalBackend, Memory, MemoryConfig, MemoryEntry, MemoryError, MemoryFileHeader,
    MemoryType,
};

pub struct FileMemory {
    pub(crate) backend: Arc<dyn Backend>,
    pub config: MemoryConfig,
    pub(crate) index_lock: Mutex<()>,
    root_dir: String,
}

impl FileMemory {
    pub fn new(workdir: &str, config: MemoryConfig, backend: Option<Arc<dyn Backend>>) -> Self {
        let backend: Arc<dyn Backend> = backend.unwrap_or_else(|| Arc::new(LocalBackend::new()));
        let root_dir = if backend.isabs(&config.memory_dir) {
            backend.normpath(&config.memory_dir)
        } else {
            backend.normpath(&backend.join_path(workdir, &config.memory_dir))
        };
        Self {
            backend,
            config,
            index_lock: Mutex::new(()),
            root_dir,
        }
    }

    pub fn root_dir(&self) -> &str {
        &self.root_dir
    }

    pub fn index_path(&self) -> String {
        self.backend.join_path(&self.root_dir, "MEMORY.md")
    }

    pub fn memory_path(&self, name: &str) -> String {
        self.backend
            .join_path(&self.root_dir, &format!("{name}.md"))
    }

    fn parse_entry(&self, filename: &str, content: &str) -> Option<MemoryEntry> {
        let fields = parse_frontmatter_fields(content);
        let name = fields.get("name")?.to_string();
        let description = fields.get("description")?.to_string();
        let mem_type = fields
            .get("type")
            .map(|value| MemoryType::from(value.as_str()))
            .unwrap_or(MemoryType::Unknown("unknown".into()));
        let mut metadata = crate::MemoryMetadata::new(mem_type);
        if let Some(created_at) = fields.get("created_at") {
            metadata.created_at = created_at.clone();
        }
        if let Some(updated_at) = fields.get("updated_at") {
            metadata.updated_at = updated_at.clone();
        }
        metadata.tags = fields.get("tags").map(|tags| {
            tags.split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToString::to_string)
                .collect()
        });
        let body = body_after_frontmatter(content)?;
        if name.is_empty() || description.is_empty() {
            debug!(
                filename,
                "skipping memory with missing required frontmatter"
            );
            return None;
        }
        Some(MemoryEntry {
            name,
            description,
            metadata,
            content: body,
        })
    }

    async fn parse_header(&self, path: String) -> Option<MemoryFileHeader> {
        let filename = std::path::Path::new(&path)
            .file_name()?
            .to_string_lossy()
            .into_owned();
        if !filename.ends_with(".md") || filename == "MEMORY.md" {
            return None;
        }
        let bytes = self.backend.read_file(&path).await.ok()?;
        let content = String::from_utf8_lossy(&bytes);
        let fields = parse_frontmatter_fields(&content);
        if fields.is_empty() {
            return None;
        }
        let description = fields.get("description").cloned();
        let mem_type = fields.get("type").map(|t| MemoryType::from(t.as_str()));
        let mtime = self.backend.stat_mtime(&path).await.ok().flatten();
        Some(MemoryFileHeader {
            filename,
            path,
            description,
            mem_type,
            mtime,
        })
    }
}

#[async_trait::async_trait]
impl Memory for FileMemory {
    #[tracing::instrument(skip(self, entry), fields(memory.name = %entry.name, memory.type = %entry.metadata.mem_type.as_str()))]
    async fn write(&self, mut entry: MemoryEntry) -> Result<(), MemoryError> {
        info!(memory.name = %entry.name, "writing memory entry");
        validate_entry(&entry)?;
        entry.metadata.updated_at = chrono::Utc::now().to_rfc3339();
        let path = self.memory_path(&entry.name);
        let serialized = serialize_frontmatter(&entry);
        self.backend
            .write_file(&path, serialized.as_bytes())
            .await?;
        let _guard = self.index_lock.lock().await;
        index::write_index_line(
            self.backend.as_ref(),
            &self.index_path(),
            &entry.name,
            &entry.description,
        )
        .await
    }

    #[tracing::instrument(skip(self))]
    async fn read(&self, name: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        debug!(memory.name = name, "reading memory entry");
        let path = self.memory_path(name);
        if !self.backend.file_exists(&path).await? {
            return Ok(None);
        }
        let bytes = self.backend.read_file(&path).await?;
        let content = String::from_utf8_lossy(&bytes);
        Ok(self.parse_entry(name, &content))
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&self, name: &str) -> Result<(), MemoryError> {
        info!(memory.name = name, "deleting memory entry");
        let path = self.memory_path(name);
        self.backend.delete_file(&path).await?;
        let _guard = self.index_lock.lock().await;
        index::remove_index_line(self.backend.as_ref(), &self.index_path(), name).await
    }

    #[tracing::instrument(skip(self))]
    async fn list(&self) -> Result<Vec<MemoryFileHeader>, MemoryError> {
        debug!("listing memory headers");
        let paths = self.backend.list_dir(&self.root_dir, false).await?;
        let mut headers = Vec::new();
        for path in paths {
            if let Some(header) = self.parse_header(path).await {
                headers.push(header);
            }
        }
        headers.sort_by(|a, b| {
            b.mtime
                .partial_cmp(&a.mtime)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        headers.truncate(self.config.retrieval_max_files);
        Ok(headers)
    }

    #[tracing::instrument(skip(self))]
    async fn search(
        &self,
        query: &str,
        type_filter: Option<MemoryType>,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        debug!(query, "searching memories");
        let needle = query.to_ascii_lowercase();
        let mut results = Vec::new();
        for header in self.list().await? {
            let name = header.filename.trim_end_matches(".md");
            if let Some(entry) = self.read(name).await? {
                if let Some(filter) = &type_filter
                    && entry.metadata.mem_type != *filter
                {
                    continue;
                }
                let haystack =
                    format!("{}\n{}", entry.description, entry.content).to_ascii_lowercase();
                if haystack.contains(&needle) {
                    results.push(entry);
                }
            }
        }
        Ok(results)
    }

    #[tracing::instrument(skip(self))]
    async fn get_index_content(&self) -> Result<String, MemoryError> {
        debug!("reading memory index");
        index::read_index(self.backend.as_ref(), &self.index_path()).await
    }

    #[tracing::instrument(skip(self, model))]
    async fn retrieve_relevant(
        &self,
        query: &str,
        model: &Arc<dyn agent_scope_model::ChatModel>,
        max_results: usize,
    ) -> Result<Option<String>, MemoryError> {
        crate::retrieval::retrieve_relevant_files(self, query, model, max_results).await
    }
}

fn validate_entry(entry: &MemoryEntry) -> Result<(), MemoryError> {
    let name_re = Regex::new(r"^[A-Za-z0-9_-]+$").map_err(|err| MemoryError::ValidationError {
        field: "name".into(),
        message: err.to_string(),
    })?;
    if entry.name.trim().is_empty() {
        return Err(MemoryError::ValidationError {
            field: "name".into(),
            message: "name must not be empty".into(),
        });
    }
    if !name_re.is_match(&entry.name) {
        return Err(MemoryError::ValidationError {
            field: "name".into(),
            message: "name must match [A-Za-z0-9_-]+".into(),
        });
    }
    if entry.description.trim().is_empty() {
        return Err(MemoryError::ValidationError {
            field: "description".into(),
            message: "description must not be empty".into(),
        });
    }
    Ok(())
}
