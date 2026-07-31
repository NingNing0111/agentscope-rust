//! Skill loader abstractions — [`SkillLoader`] trait, [`LocalSkillLoader`], and
//! [`SkillOrLoader`] enum for registering skills with [`ToolKit`](crate::ToolKit).

use std::collections::HashMap;

use agent_scope_workspace::Skill;

// ---------------------------------------------------------------------------
// SkillLoader trait (T004)
// ---------------------------------------------------------------------------

/// Abstract loader that can provide [`Skill`] instances from any source
/// (local filesystem, remote service, MCP, etc.).
///
/// # Thread Safety
/// `Send + Sync` — safe to share via `Arc<dyn SkillLoader>`.
///
/// # Contract guarantees
///
/// | Guarantee | Detail |
/// |-----------|--------|
/// | Thread safety | `Send + Sync` |
/// | Graceful degradation | Implementations return `[]` on I/O errors, never panic |
/// | No mutable state | `&self` — caches behind `RwLock` if needed |
/// | No unsafe | All implementations MUST be safe Rust |
#[async_trait::async_trait]
pub trait SkillLoader: Send + Sync {
    /// Return all skills this loader can provide.
    ///
    /// # Errors
    /// Implementations SHOULD return an empty `Vec` rather than error
    /// when the source is temporarily unavailable, logging a warning.
    async fn list_skills(&self) -> Vec<Skill>;
}

// ---------------------------------------------------------------------------
// SkillOrLoader enum (T005)
// ---------------------------------------------------------------------------

/// Tagged union representing the three ways to register a skill source.
///
/// | Variant | Use case |
/// |---------|----------|
/// | `Skill(Skill)` | Directly pass a pre-built [`Skill`] |
/// | `Loader(Box<dyn SkillLoader>)` | Any custom [`SkillLoader`] impl |
/// | `Dir(String)` | Directory path → internally wraps in [`LocalSkillLoader`] |
pub enum SkillOrLoader {
    /// Directly passed [`Skill`] object.
    Skill(Skill),
    /// A custom [`SkillLoader`] trait object.
    Loader(Box<dyn SkillLoader>),
    /// A file-system directory path — lazily converted to [`LocalSkillLoader`].
    Dir(String),
}

// ---------------------------------------------------------------------------
// parse_skill_md helper (T006) — duplicated from agent_scope_workspace::skill
// since the original is pub(crate).
// ---------------------------------------------------------------------------

/// Parse a SKILL.md file's YAML frontmatter and body.
///
/// Expected format:
/// ```markdown
/// ---
/// name: skill-name
/// description: A description of the skill
/// ---
///
/// Markdown body here...
/// ```
///
/// Returns `(name, description, body)`.  Missing or malformed frontmatter
/// results in empty strings for name/description.
pub(crate) fn parse_skill_md(content: &str) -> (String, String, String) {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return (String::new(), String::new(), content.to_string());
    }
    let rest = &trimmed[3..];
    let end = rest.find("---").unwrap_or(rest.len());
    let frontmatter = &rest[..end];
    let body_start = std::cmp::min(end + 3, rest.len());
    let body = rest[body_start..].trim().to_string();

    let mut name = String::new();
    let mut description = String::new();

    for line in frontmatter.lines() {
        if let Some(value) = line.strip_prefix("name:") {
            name = value.trim().trim_matches('"').to_string();
        } else if let Some(value) = line.strip_prefix("description:") {
            description = value.trim().trim_matches('"').to_string();
        }
    }

    (name, description, body)
}

// ---------------------------------------------------------------------------
// LocalSkillLoader (T007, T008)
// ---------------------------------------------------------------------------

/// Scans a local directory tree for `SKILL.md` files and parses them into
/// [`Skill`] instances.
///
/// Caches results by file mtime to avoid re-reading unchanged files.
///
/// # Examples
///
/// ```text
/// let loader = LocalSkillLoader::new("/path/to/skills", true);
/// let skills = loader.list_skills().await;
/// ```
pub struct LocalSkillLoader {
    /// Absolute path to the root directory to scan.
    directory: String,
    /// Whether to recursively scan subdirectories.
    scan_subdir: bool,
    /// Cache: key = directory path containing SKILL.md, value = (mtime, Skill).
    _cache: std::sync::Mutex<HashMap<String, (f64, Skill)>>,
}

impl LocalSkillLoader {
    /// Creates a new [`LocalSkillLoader`].
    ///
    /// # Arguments
    /// * `directory` — Absolute path to the root directory to scan.
    /// * `scan_subdir` — If `true`, recursively scan subdirectories for
    ///   `SKILL.md` files.
    #[must_use]
    pub fn new(directory: &str, scan_subdir: bool) -> Self {
        Self {
            directory: directory.to_string(),
            scan_subdir,
            _cache: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl SkillLoader for LocalSkillLoader {
    async fn list_skills(&self) -> Vec<Skill> {
        let dir_path = std::path::Path::new(&self.directory);

        // Directory doesn't exist → empty result (T008 edge case)
        if !dir_path.exists() {
            tracing::warn!("skill directory does not exist: {}", self.directory);
            return Vec::new();
        }
        if !dir_path.is_dir() {
            tracing::warn!("skill path is not a directory: {}", self.directory);
            return Vec::new();
        }

        let mut cache = self._cache.lock().unwrap_or_else(|e| e.into_inner());

        // Discover SKILL.md files
        let skill_dirs = discover_skill_dirs(dir_path, self.scan_subdir);

        let mut results: Vec<Skill> = Vec::new();
        let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

        for dir in &skill_dirs {
            let skill_md_path = dir.join("SKILL.md");

            // Check mtime for cache
            let mtime = match std::fs::metadata(&skill_md_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
            {
                Some(mt) => mt,
                None => {
                    tracing::warn!("failed to read mtime for {:?}", skill_md_path);
                    continue;
                }
            };

            let dir_key = dir.to_string_lossy().to_string();

            // Cache hit: same mtime → reuse cached Skill
            if let Some((cached_mtime, cached_skill)) = cache.get(&dir_key)
                && (mtime - cached_mtime).abs() < f64::EPSILON
            {
                results.push(cached_skill.clone());
                seen_dirs.insert(dir_key);
                continue;
            }

            // Cache miss: read and parse
            match std::fs::read_to_string(&skill_md_path) {
                Ok(content) => {
                    let (name, description, markdown) = parse_skill_md(&content);

                    // Missing name or description → skip with warning
                    if name.is_empty() || description.is_empty() {
                        tracing::warn!(
                            "SKILL.md at {:?} missing name or description, skipping",
                            dir
                        );
                        continue;
                    }

                    let skill = Skill {
                        name,
                        description,
                        dir: dir.to_string_lossy().to_string(),
                        markdown,
                        updated_at: mtime,
                    };

                    cache.insert(dir_key.clone(), (mtime, skill.clone()));
                    results.push(skill);
                    seen_dirs.insert(dir_key);
                }
                Err(e) => {
                    tracing::warn!("failed to read SKILL.md at {:?}: {e}", skill_md_path);
                    continue;
                }
            }
        }

        // Evict cache entries for directories that no longer exist
        cache.retain(|k, _| seen_dirs.contains(k));

        results
    }
}

/// Discover directories containing a `SKILL.md` file.
fn discover_skill_dirs(root: &std::path::Path, scan_subdir: bool) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();

    // Check root first
    let root_skill = root.join("SKILL.md");
    if root_skill.exists() && root_skill.is_file() {
        result.push(root.to_path_buf());
    }

    // Scan subdirectories if enabled
    if scan_subdir && let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() && skill_md.is_file() {
                    result.push(path);
                } else {
                    let mut nested = discover_skill_dirs(&path, true);
                    result.append(&mut nested);
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_md_valid() {
        let content =
            "---\nname: test-skill\ndescription: A test skill\n---\n\n# Hello\n\nSome body.";
        let (name, desc, body) = parse_skill_md(content);
        assert_eq!(name, "test-skill");
        assert_eq!(desc, "A test skill");
        assert!(body.contains("# Hello"));
    }

    #[test]
    fn test_parse_skill_md_no_frontmatter() {
        let content = "# Just markdown\n\nNo frontmatter here.";
        let (name, desc, body) = parse_skill_md(content);
        assert!(name.is_empty());
        assert!(desc.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_skill_md_missing_end_delimiter() {
        let content = "---\nname: foo\ndescription: bar\n---";
        let (name, desc, body) = parse_skill_md(content);
        assert_eq!(name, "foo");
        assert_eq!(desc, "bar");
        assert!(body.is_empty());
    }

    #[test]
    fn test_parse_skill_md_quoted_values() {
        let content = "---\nname: \"my-skill\"\ndescription: \"My description\"\n---\n\nBody here.";
        let (name, desc, body) = parse_skill_md(content);
        assert_eq!(name, "my-skill");
        assert_eq!(desc, "My description");
        assert!(body.contains("Body here"));
    }
}
