//! Turbovec-backed vector store implementation.
//!
//! This module integrates the [`turbovec`] `IdMapIndex` with AgentScope's
//! async [`VectorStore`](crate::vector_store::VectorStore) trait. Turbovec is a
//! local, in-process vector index that compresses each coordinate to 2, 3, or 4
//! bits. Lower bit widths use less memory; higher bit widths generally improve
//! recall.
//!
//! # Example
//!
//! ```no_run
//! # use agent_scope_rag::turbovec_store::TurbovecVectorStore;
//! let store = TurbovecVectorStore::new(4)?;
//! # Ok::<(), agent_scope_rag::error::VectorStoreError>(())
//! ```

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tokio::task;
use turbovec::{AddError, IdMapIndex};

use crate::chunker::Chunk;
use crate::error::VectorStoreError;
use crate::vector_store::{DocumentSummary, VectorRecord, VectorSearchResult, VectorStore};

const MANIFEST_VERSION: u32 = 1;

/// TQ+ calibration state for a turbovec collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationState {
    /// Fewer than 1000 vectors have been added and calibration is still warming up.
    WarmingUp,
    /// A fitted calibration has been committed.
    Fitted,
    /// The index is committed to identity calibration.
    Identity,
}

/// Local vector store backed by turbovec `IdMapIndex` collections.
pub struct TurbovecVectorStore {
    bit_width: usize,
    collections: tokio::sync::RwLock<HashMap<String, Arc<RwLock<CollectionInner>>>>,
}

struct CollectionInner {
    dim: usize,
    index: IdMapIndex,
    chunk_meta: HashMap<u64, ChunkMetaEntry>,
    doc_index: HashMap<String, Vec<u64>>,
    next_internal_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkMetaEntry {
    document_id: String,
    chunk_index: usize,
    total_chunks: usize,
    source: String,
    content: String,
    metadata: HashMap<String, String>,
}

type CollectionMetaRead = (HashMap<u64, ChunkMetaEntry>, HashMap<String, Vec<u64>>, u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreManifest {
    version: u32,
    bit_width: usize,
    collections: HashMap<String, CollectionManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionManifestEntry {
    dim: usize,
    n_vectors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionMetaFile {
    chunks: HashMap<String, ChunkMetaEntry>,
}

impl TurbovecVectorStore {
    /// Create an empty turbovec vector store.
    ///
    /// `bit_width` must be 2, 3, or 4. Higher values use more memory and
    /// generally preserve vector similarity more accurately.
    pub fn new(bit_width: usize) -> Result<Self, VectorStoreError> {
        validate_bit_width(bit_width)?;
        Ok(Self {
            bit_width,
            collections: tokio::sync::RwLock::new(HashMap::new()),
        })
    }

    /// Persist the entire store to `path`.
    ///
    /// The directory is created if it does not exist. Each collection writes a
    /// `.tvim` index file and a `.meta` JSON metadata file, plus a store-level
    /// `manifest.json`.
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<(), VectorStoreError> {
        let path = path.as_ref().to_path_buf();
        let bit_width = self.bit_width;
        let collections: Vec<(String, Arc<RwLock<CollectionInner>>)> = {
            let guard = self.collections.read().await;
            guard
                .iter()
                .map(|(name, inner)| (name.clone(), Arc::clone(inner)))
                .collect()
        };

        task::spawn_blocking(move || {
            fs::create_dir_all(&path).map_err(io_error)?;
            let mut manifest = StoreManifest {
                version: MANIFEST_VERSION,
                bit_width,
                collections: HashMap::new(),
            };

            for (name, collection) in collections {
                let guard = collection
                    .read()
                    .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
                let tvim_path = collection_path(&path, &name, "tvim");
                let meta_path = collection_path(&path, &name, "meta");
                guard.index.write(&tvim_path).map_err(io_error)?;
                write_collection_meta(&meta_path, &guard.chunk_meta)?;
                manifest.collections.insert(
                    name,
                    CollectionManifestEntry {
                        dim: guard.dim,
                        n_vectors: guard.index.len(),
                    },
                );
            }

            manifest.write(&path)
        })
        .await
        .map_err(join_error)??;
        Ok(())
    }

    /// Load a store previously written by [`Self::save`].
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, VectorStoreError> {
        let path = path.as_ref().to_path_buf();
        task::spawn_blocking(move || {
            let manifest = StoreManifest::read(&path)?;
            validate_bit_width(manifest.bit_width)?;
            let mut collections = HashMap::new();

            for (name, entry) in &manifest.collections {
                let tvim_path = collection_path(&path, name, "tvim");
                let meta_path = collection_path(&path, name, "meta");
                let index = IdMapIndex::load(&tvim_path).map_err(io_error)?;
                if index.len() != entry.n_vectors {
                    return Err(VectorStoreError::BackendError(
                        "corrupted: vector count mismatch".to_string(),
                    ));
                }
                let (chunk_meta, doc_index, next_internal_id) = read_collection_meta(&meta_path)?;
                collections.insert(
                    name.clone(),
                    Arc::new(RwLock::new(CollectionInner {
                        dim: entry.dim,
                        index,
                        chunk_meta,
                        doc_index,
                        next_internal_id,
                    })),
                );
            }

            Ok(Self {
                bit_width: manifest.bit_width,
                collections: tokio::sync::RwLock::new(collections),
            })
        })
        .await
        .map_err(join_error)?
    }

    /// Query the turbovec TQ+ calibration state for `collection`.
    pub async fn calibration_state(
        &self,
        collection: &str,
    ) -> Result<CalibrationState, VectorStoreError> {
        let inner = self.collection(collection).await?;
        task::spawn_blocking(move || {
            let guard = inner
                .read()
                .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
            Ok(map_calibration_state(guard.index.len()))
        })
        .await
        .map_err(join_error)?
    }

    async fn collection(
        &self,
        name: &str,
    ) -> Result<Arc<RwLock<CollectionInner>>, VectorStoreError> {
        let guard = self.collections.read().await;
        guard
            .get(name)
            .cloned()
            .ok_or_else(|| VectorStoreError::CollectionNotFound(name.to_string()))
    }

    async fn ensure_collection(
        &self,
        name: &str,
        dim: usize,
    ) -> Result<Arc<RwLock<CollectionInner>>, VectorStoreError> {
        validate_dim(dim)?;
        {
            let guard = self.collections.read().await;
            if let Some(inner) = guard.get(name) {
                return Ok(Arc::clone(inner));
            }
        }

        let mut guard = self.collections.write().await;
        if let Some(inner) = guard.get(name) {
            return Ok(Arc::clone(inner));
        }
        let index =
            IdMapIndex::new(dim, self.bit_width).map_err(|e| map_turbovec_error(e, dim, dim))?;
        let inner = Arc::new(RwLock::new(CollectionInner {
            dim,
            index,
            chunk_meta: HashMap::new(),
            doc_index: HashMap::new(),
            next_internal_id: 1,
        }));
        guard.insert(name.to_string(), Arc::clone(&inner));
        Ok(inner)
    }
}

#[async_trait::async_trait]
impl VectorStore for TurbovecVectorStore {
    /// Check whether a collection exists.
    async fn has_collection(&self, name: &str) -> Result<bool, VectorStoreError> {
        let guard = self.collections.read().await;
        Ok(guard.contains_key(name))
    }

    /// Create a collection with a fixed vector dimension.
    async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), VectorStoreError> {
        let dim = dimensions as usize;
        validate_dim(dim)?;
        let mut guard = self.collections.write().await;
        if guard.contains_key(name) {
            return Err(VectorStoreError::CollectionAlreadyExists(name.to_string()));
        }
        let index =
            IdMapIndex::new(dim, self.bit_width).map_err(|e| map_turbovec_error(e, dim, dim))?;
        guard.insert(
            name.to_string(),
            Arc::new(RwLock::new(CollectionInner {
                dim,
                index,
                chunk_meta: HashMap::new(),
                doc_index: HashMap::new(),
                next_internal_id: 1,
            })),
        );
        Ok(())
    }

    /// Search similar vectors, optionally constrained by exact-match metadata.
    async fn search(
        &self,
        collection: &str,
        mut query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError> {
        if top_k == 0 {
            return Ok(Vec::new());
        }
        let inner = self.collection(collection).await?;
        task::spawn_blocking(move || {
            let guard = inner
                .read()
                .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
            if query_vector.len() != guard.dim {
                return Err(VectorStoreError::DimensionMismatch {
                    expected: guard.dim as u32,
                    got: query_vector.len(),
                });
            }
            if guard.index.is_empty() {
                return Ok(Vec::new());
            }
            l2_normalize(&mut query_vector)?;
            let allowlist = metadata_filter
                .as_ref()
                .map(|filter| build_allowlist(&guard.chunk_meta, filter));
            if matches!(allowlist.as_ref(), Some(ids) if ids.is_empty()) {
                return Ok(Vec::new());
            }
            let (scores, ids) =
                guard
                    .index
                    .search_with_allowlist(&query_vector, top_k, allowlist.as_deref());
            let mut results = Vec::with_capacity(ids.len());
            for (score, id) in scores.into_iter().zip(ids) {
                if let Some(result) = build_search_result(score, id, &guard.chunk_meta) {
                    results.push(result);
                }
            }
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            Ok(results)
        })
        .await
        .map_err(join_error)?
    }

    /// Insert vector records, auto-creating the collection from the first record if needed.
    async fn insert(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError> {
        if records.is_empty() {
            return Ok(());
        }
        let dim = records[0].vector.len();
        let inner = self.ensure_collection(collection, dim).await?;
        task::spawn_blocking(move || {
            let mut guard = inner
                .write()
                .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
            let dim = guard.dim;
            let mut flat = Vec::with_capacity(records.len() * dim);
            let mut ids = Vec::with_capacity(records.len());
            let mut metas = Vec::with_capacity(records.len());

            for record in records {
                if record.vector.len() != dim {
                    return Err(VectorStoreError::DimensionMismatch {
                        expected: dim as u32,
                        got: record.vector.len(),
                    });
                }
                let mut vector = record.vector;
                l2_normalize(&mut vector)?;
                flat.extend_from_slice(&vector);

                let mut id = generate_internal_id(&record.document_id, record.chunk.chunk_index);
                if guard.chunk_meta.contains_key(&id) {
                    remove_id(&mut guard, id);
                }
                while ids.contains(&id) || guard.chunk_meta.contains_key(&id) {
                    guard.next_internal_id = guard.next_internal_id.saturating_add(1);
                    id = guard.next_internal_id;
                }
                ids.push(id);
                metas.push((
                    id,
                    ChunkMetaEntry {
                        document_id: record.document_id,
                        chunk_index: record.chunk.chunk_index,
                        total_chunks: record.chunk.total_chunks,
                        source: record.chunk.source,
                        content: record.chunk.content,
                        metadata: record.chunk.metadata,
                    },
                ));
            }

            guard
                .index
                .add_with_ids(&flat, &ids)
                .map_err(|e| map_turbovec_error(e, dim, flat.len()))?;
            for (id, meta) in metas {
                guard
                    .doc_index
                    .entry(meta.document_id.clone())
                    .or_default()
                    .push(id);
                guard.chunk_meta.insert(id, meta);
            }
            Ok(())
        })
        .await
        .map_err(join_error)?
    }

    /// Delete all chunks belonging to a document.
    async fn delete(&self, collection: &str, document_id: &str) -> Result<(), VectorStoreError> {
        let inner = self.collection(collection).await?;
        let document_id = document_id.to_string();
        task::spawn_blocking(move || {
            let mut guard = inner
                .write()
                .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
            if let Some(ids) = guard.doc_index.remove(&document_id) {
                for id in ids.into_iter().rev() {
                    guard.index.remove(id);
                    guard.chunk_meta.remove(&id);
                }
            }
            Ok(())
        })
        .await
        .map_err(join_error)?
    }

    /// List distinct documents in a collection.
    async fn list_documents(
        &self,
        collection: &str,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<DocumentSummary>, VectorStoreError> {
        let inner = self.collection(collection).await?;
        task::spawn_blocking(move || {
            let guard = inner
                .read()
                .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
            let mut summaries: HashMap<String, DocumentSummary> = HashMap::new();
            for meta in guard.chunk_meta.values() {
                if let Some(ref filter) = metadata_filter
                    && !metadata_matches(&meta.metadata, filter)
                {
                    continue;
                }
                let entry = summaries
                    .entry(meta.document_id.clone())
                    .or_insert_with(|| DocumentSummary {
                        document_id: meta.document_id.clone(),
                        source: meta.source.clone(),
                        chunk_count: 0,
                        metadata: meta.metadata.clone(),
                    });
                entry.chunk_count += 1;
            }
            let mut docs: Vec<_> = summaries.into_values().collect();
            docs.sort_by(|a, b| a.document_id.cmp(&b.document_id));
            Ok(docs)
        })
        .await
        .map_err(join_error)?
    }
}

impl StoreManifest {
    fn write(&self, path: &Path) -> Result<(), VectorStoreError> {
        let final_path = path.join("manifest.json");
        atomic_write_json(&final_path, self)
    }

    fn read(path: &Path) -> Result<Self, VectorStoreError> {
        let manifest_path = path.join("manifest.json");
        let bytes = fs::read(&manifest_path).map_err(io_error)?;
        let manifest: Self = serde_json::from_slice(&bytes)
            .map_err(|e| VectorStoreError::BackendError(format!("manifest parse error: {e}")))?;
        if manifest.version > MANIFEST_VERSION {
            return Err(VectorStoreError::BackendError(format!(
                "unsupported manifest version: {}",
                manifest.version
            )));
        }
        validate_bit_width(manifest.bit_width)?;
        Ok(manifest)
    }
}

fn generate_internal_id(document_id: &str, chunk_index: usize) -> u64 {
    let mut hasher = DefaultHasher::new();
    document_id.hash(&mut hasher);
    chunk_index.hash(&mut hasher);
    hasher.finish()
}

fn l2_normalize(vec: &mut [f32]) -> Result<(), VectorStoreError> {
    for value in vec.iter() {
        if !value.is_finite() {
            return Err(VectorStoreError::BackendError(
                "vector contains non-finite value".to_string(),
            ));
        }
    }
    let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        for value in vec {
            *value /= norm;
        }
    }
    Ok(())
}

fn map_turbovec_error(err: impl Error, expected: usize, got: usize) -> VectorStoreError {
    let msg = err.to_string();
    if msg.contains("DimMismatch") || msg.contains("dim mismatch") || msg.contains("dimension") {
        VectorStoreError::DimensionMismatch {
            expected: expected as u32,
            got,
        }
    } else {
        VectorStoreError::BackendError(msg)
    }
}

fn validate_bit_width(bit_width: usize) -> Result<(), VectorStoreError> {
    if (2..=4).contains(&bit_width) {
        Ok(())
    } else {
        Err(VectorStoreError::BackendError(format!(
            "bit_width must be 2, 3, or 4, got {bit_width}"
        )))
    }
}

fn validate_dim(dim: usize) -> Result<(), VectorStoreError> {
    if dim == 0 || !dim.is_multiple_of(8) || dim > 16384 {
        Err(VectorStoreError::BackendError(format!(
            "dimension must be positive, multiple of 8, and <= 16384, got {dim}"
        )))
    } else {
        Ok(())
    }
}

fn build_allowlist(
    chunk_meta: &HashMap<u64, ChunkMetaEntry>,
    filter: &HashMap<String, String>,
) -> Vec<u64> {
    chunk_meta
        .iter()
        .filter_map(|(id, meta)| metadata_matches(&meta.metadata, filter).then_some(*id))
        .collect()
}

fn metadata_matches(metadata: &HashMap<String, String>, filter: &HashMap<String, String>) -> bool {
    filter
        .iter()
        .all(|(key, value)| metadata.get(key) == Some(value))
}

fn build_search_result(
    score: f32,
    internal_id: u64,
    chunk_meta: &HashMap<u64, ChunkMetaEntry>,
) -> Option<VectorSearchResult> {
    let meta = chunk_meta.get(&internal_id)?;
    Some(VectorSearchResult {
        score,
        document_id: meta.document_id.clone(),
        chunk: Chunk {
            content: meta.content.clone(),
            source: meta.source.clone(),
            chunk_index: meta.chunk_index,
            total_chunks: meta.total_chunks,
            metadata: meta.metadata.clone(),
        },
    })
}

fn remove_id(collection: &mut CollectionInner, id: u64) {
    collection.index.remove(id);
    if let Some(meta) = collection.chunk_meta.remove(&id)
        && let Some(ids) = collection.doc_index.get_mut(&meta.document_id)
    {
        ids.retain(|existing| *existing != id);
        if ids.is_empty() {
            collection.doc_index.remove(&meta.document_id);
        }
    }
}

fn write_collection_meta(
    path: &Path,
    chunk_meta: &HashMap<u64, ChunkMetaEntry>,
) -> Result<(), VectorStoreError> {
    let chunks = chunk_meta
        .iter()
        .map(|(id, meta)| (id.to_string(), meta.clone()))
        .collect();
    atomic_write_json(path, &CollectionMetaFile { chunks })
}

fn read_collection_meta(path: &Path) -> Result<CollectionMetaRead, VectorStoreError> {
    let bytes = fs::read(path).map_err(io_error)?;
    let meta_file: CollectionMetaFile = serde_json::from_slice(&bytes)
        .map_err(|e| VectorStoreError::BackendError(format!("metadata parse error: {e}")))?;
    let mut chunk_meta = HashMap::new();
    let mut doc_index: HashMap<String, Vec<u64>> = HashMap::new();
    let mut max_id = 0_u64;

    for (id_text, meta) in meta_file.chunks {
        let id = id_text
            .parse::<u64>()
            .map_err(|e| VectorStoreError::BackendError(format!("invalid metadata id: {e}")))?;
        max_id = max_id.max(id);
        doc_index
            .entry(meta.document_id.clone())
            .or_default()
            .push(id);
        chunk_meta.insert(id, meta);
    }

    Ok((chunk_meta, doc_index, max_id.saturating_add(1)))
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), VectorStoreError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| VectorStoreError::BackendError(format!("json serialize error: {e}")))?;
    let tmp_path = tmp_path(path);
    fs::write(&tmp_path, bytes).map_err(io_error)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .open(&tmp_path)
        .map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    fs::rename(&tmp_path, path).map_err(io_error)?;
    Ok(())
}

fn collection_path(base: &Path, name: &str, extension: &str) -> PathBuf {
    base.join(format!("{name}.{extension}"))
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(".tmp");
    PathBuf::from(os)
}

fn io_error(err: std::io::Error) -> VectorStoreError {
    VectorStoreError::BackendError(err.to_string())
}

fn join_error(err: task::JoinError) -> VectorStoreError {
    VectorStoreError::BackendError(format!("join error: {err}"))
}

fn map_calibration_state(len: usize) -> CalibrationState {
    if len >= 1000 {
        CalibrationState::Fitted
    } else {
        CalibrationState::WarmingUp
    }
}

#[allow(dead_code)]
fn _assert_add_error_is_error(error: AddError) -> VectorStoreError {
    map_turbovec_error(error, 0, 0)
}
