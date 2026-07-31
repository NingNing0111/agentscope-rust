//! Tests for the FileEmbeddingCache.

use agent_scope_embedding::FileEmbeddingCache;
use agent_scope_embedding::cache::EmbeddingCache;
use agent_scope_embedding::cache::hash_key;

#[test]
fn test_cache_hit() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");

    cache.store("key1", vec![vec![1.0, 2.0]]);
    let result = cache.lookup("key1");
    assert!(result.is_some());
    let result = result.expect("cache hit");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], vec![1.0_f32, 2.0]);
}

#[test]
fn test_cache_miss() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");
    assert!(cache.lookup("nonexistent").is_none());
}

#[test]
fn test_cache_overwrite() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");

    cache.store("key", vec![vec![1.0]]);
    cache.store("key", vec![vec![5.0, 6.0]]);
    let result = cache.lookup("key").expect("cache hit");
    assert_eq!(result, vec![vec![5.0_f32, 6.0]]);
}

#[test]
fn test_cache_100_entries() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cache = FileEmbeddingCache::new(dir.path().to_path_buf()).expect("create cache");

    for i in 0..100 {
        cache.store(&format!("key_{i}"), vec![vec![i as f32]]);
    }

    for i in 0..100 {
        let result = cache
            .lookup(&format!("key_{i}"))
            .unwrap_or_else(|| panic!("cache miss for key_{i}"));
        assert!((result[0][0] - i as f32).abs() < f32::EPSILON);
    }
}

#[test]
fn test_hash_key_deterministic() {
    assert_eq!(hash_key("hello world"), hash_key("hello world"));
}

#[test]
fn test_hash_key_different() {
    assert_ne!(hash_key("hello"), hash_key("world"));
}
