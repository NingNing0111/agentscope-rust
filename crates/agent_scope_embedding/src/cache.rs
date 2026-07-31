//! Embedding cache trait and file-based implementation.
//!
//! Provides the [`EmbeddingCache`] trait for caching embedding results,
//! and [`FileEmbeddingCache`] — a file-system backed implementation.

use std::io;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// EmbeddingCache trait
// ---------------------------------------------------------------------------

/// Trait for embedding result caches.
///
/// The key is typically a SHA-256 hash of the input content.
/// This allows content-addressable caching — identical inputs always
/// produce the same cache key.
pub trait EmbeddingCache: Send + Sync {
    /// Look up cached embeddings by key.
    ///
    /// Returns `None` on cache miss, `Some(embeddings)` on hit.
    fn lookup(&self, key: &str) -> Option<Vec<Vec<f32>>>;

    /// Store embeddings under a key.
    ///
    /// Overwrites any existing entry with the same key.
    fn store(&self, key: &str, embeddings: Vec<Vec<f32>>);
}

// ---------------------------------------------------------------------------
// FileEmbeddingCache
// ---------------------------------------------------------------------------

/// File-system backed embedding cache.
///
/// Each key is stored as `{cache_dir}/{key}.json`.
/// The file contains a JSON array of `Vec<Vec<f32>>`.
pub struct FileEmbeddingCache {
    cache_dir: PathBuf,
}

impl FileEmbeddingCache {
    /// Create a new file-based cache rooted at `cache_dir`.
    ///
    /// The directory is created (recursively) if it does not exist.
    pub fn new(cache_dir: PathBuf) -> io::Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Return a reference to the cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Generate the file path for a given cache key.
    fn key_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{key}.json"))
    }
}

impl EmbeddingCache for FileEmbeddingCache {
    fn lookup(&self, key: &str) -> Option<Vec<Vec<f32>>> {
        let path = self.key_path(key);
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<Vec<Vec<f32>>>(&data).ok()
    }

    fn store(&self, key: &str, embeddings: Vec<Vec<f32>>) {
        let path = self.key_path(key);
        if let Ok(json) = serde_json::to_string(&embeddings) {
            let _ = std::fs::write(path, json);
        }
    }
}

// ---------------------------------------------------------------------------
// SHA-256 key generation utility
// ---------------------------------------------------------------------------

/// Generate a deterministic cache key from embedding input content.
///
/// Uses SHA-256 hashing for content-addressable lookups.
pub fn hash_key(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_embedding_cache_hit() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");

        let embeddings = vec![vec![1.0_f32, 2.0, 3.0]];
        cache.store("key1", embeddings.clone());
        let result = cache.lookup("key1");
        assert!(result.is_some());
        let result = result.expect("cache hit");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 3);
        assert!((result[0][0] - 1.0).abs() < f32::EPSILON);
        assert!((result[0][1] - 2.0).abs() < f32::EPSILON);
        assert!((result[0][2] - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_file_embedding_cache_miss() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");
        let result = cache.lookup("nonexistent_key");
        assert!(result.is_none());
    }

    #[test]
    fn test_file_embedding_cache_overwrite() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");

        let v1 = vec![vec![1.0_f32]];
        let v2 = vec![vec![5.0_f32, 6.0]];
        cache.store("key", v1);
        cache.store("key", v2.clone());
        let result = cache.lookup("key").expect("cache hit after overwrite");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 2);
    }

    #[test]
    fn test_file_embedding_cache_100_entries() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");

        for i in 0..100 {
            let key = format!("key_{i}");
            let embeddings = vec![vec![i as f32]];
            cache.store(&key, embeddings);
        }

        for i in 0..100 {
            let key = format!("key_{i}");
            let result = cache.lookup(&key);
            assert!(result.is_some(), "cache miss for key_{i}");
            let result = result.expect("cache hit");
            assert!((result[0][0] - i as f32).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_hash_key_deterministic() {
        let k1 = hash_key("hello world");
        let k2 = hash_key("hello world");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_hash_key_different_inputs() {
        let k1 = hash_key("hello");
        let k2 = hash_key("world");
        assert_ne!(k1, k2);
    }
}
