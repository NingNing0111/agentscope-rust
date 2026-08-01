# Contract: TurbovecVectorStore API

**Feature**: 016-turbovec-rag
**Trait**: `agent_scope_rag::vector_store::VectorStore`
**Implementation**: `agent_scope_rag::turbovec_store::TurbovecVectorStore`

## Construction

```rust
/// Create an empty TurbovecVectorStore.
///
/// # Parameters
/// * `bit_width` — compression bits per coordinate: 2, 3, or 4.
///   Higher = better recall at the cost of more memory.
///
/// # Errors
/// Returns error if bit_width ∉ {2, 3, 4}.
pub fn new(bit_width: usize) -> Result<Self, VectorStoreError>
```

## VectorStore Trait Methods

### has_collection

```rust
/// Check if a collection exists.
///
/// # Returns
/// * `Ok(true)` — collection exists
/// * `Ok(false)` — collection does not exist
/// * `Err(_)` — only on internal lock poisoning
async fn has_collection(&self, name: &str) -> Result<bool, VectorStoreError>
```

### create_collection

```rust
/// Create a new collection explicitly.
///
/// # Parameters
/// * `name` — case-sensitive collection name
/// * `dimensions` — vector dimensionality (must be positive, multiple of 8, ≤ 16384)
///
/// # Errors
/// * `CollectionAlreadyExists(name)` — collection with this name already present
/// * `BackendError(msg)` — dimension constraints violated or IdMapIndex construction failed
async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), VectorStoreError>
```

### search

```rust
/// Search for similar vectors.
///
/// # Parameters
/// * `collection` — collection name
/// * `query_vector` — L2-normalized query embedding (dimension must match collection)
/// * `top_k` — max results (clamped to collection size)
/// * `metadata_filter` — optional exact-match AND filter on chunk metadata
///
/// # Returns
/// Results sorted by score descending (highest similarity first).
/// Empty Vec if collection exists but is empty.
///
/// # Errors
/// * `CollectionNotFound(name)` — collection not created
/// * `DimensionMismatch` — query_vector length ≠ collection dim
/// * `BackendError(msg)` — turbovec search error or lock poisoning
async fn search(
    &self,
    collection: &str,
    query_vector: Vec<f32>,
    top_k: usize,
    metadata_filter: Option<HashMap<String, String>>,
) -> Result<Vec<VectorSearchResult>, VectorStoreError>
```

### insert

```rust
/// Insert vector records into a collection.
/// Lazy-creates collection if it does not exist (same dim for all records in batch).
///
/// # Parameters
/// * `collection` — target collection name
/// * `records` — vector records to insert (empty Vec is a no-op)
///
/// # Behavior
/// 1. If collection doesn't exist → auto-create with dimension from first record's vector.len()
/// 2. L2-normalize each vector (zero-norm vectors: stored as-is, score=0)
/// 3. Generate deterministic internal u64 IDs
/// 4. Call `IdMapIndex::add_with_ids`
/// 5. Store ChunkMeta in internal map
/// 6. Update document reverse index
///
/// # Errors
/// * `DimensionMismatch` — any vector length ≠ collection dim
/// * `BackendError(msg)` — turbovec encode error or lock poisoning
async fn insert(
    &self,
    collection: &str,
    records: Vec<VectorRecord>,
) -> Result<(), VectorStoreError>
```

### delete

```rust
/// Delete all chunks belonging to a document.
/// Idempotent — deleting a non-existent document returns Ok(()).
///
/// # Parameters
/// * `collection` — collection name
/// * `document_id` — document to delete
///
/// # Behavior
/// 1. Look up all internal IDs from doc_index
/// 2. Call `IdMapIndex::remove` for each ID (reverse order to minimize moves)
/// 3. Remove entries from chunk_meta and doc_index
///
/// # Errors
/// * `CollectionNotFound(name)` — collection doesn't exist
/// * `BackendError(msg)` — lock poisoning
async fn delete(&self, collection: &str, document_id: &str) -> Result<(), VectorStoreError>
```

### list_documents

```rust
/// List distinct documents in a collection, with optional metadata filter.
///
/// # Parameters
/// * `collection` — collection name
/// * `metadata_filter` — optional exact-match AND filter
///
/// # Returns
/// List of DocumentSummary, one per distinct document_id.
///
/// # Errors
/// * `CollectionNotFound(name)` — collection doesn't exist
/// * `BackendError(msg)` — lock poisoning
async fn list_documents(
    &self,
    collection: &str,
    metadata_filter: Option<HashMap<String, String>>,
) -> Result<Vec<DocumentSummary>, VectorStoreError>
```

## Extension Methods (not in VectorStore trait)

### calibration_state

```rust
/// Query the TQ+ calibration state of a collection's underlying index.
///
/// # Returns
/// * `WarmingUp` — <1000 vectors added, identity calibration, recall not yet optimized
/// * `Fitted` — ≥1000 vectors, real calibration applied, best recall
/// * `Identity` — loaded from a file saved while warming up, permanently fixed to identity
pub fn calibration_state(&self, collection: &str) -> Result<CalibrationState, VectorStoreError>
```

### save / load

```rust
/// Persist the entire store to a directory.
///
/// # Parameters
/// * `path` — directory path (must exist)
///
/// # Behavior
/// 1. Writes manifest.json (collection list + metadata)
/// 2. For each collection, writes `{name}.tvim` (turbovec format) + `{name}.meta` (JSON chunk metadata)
/// 3. Atomic per-file writes (temp → fsync → rename)
pub async fn save(&self, path: impl AsRef<Path>) -> Result<(), VectorStoreError>

/// Load a store from a directory previously written by save().
///
/// # Parameters
/// * `path` — directory containing manifest.json + collection files
///
/// # Returns
/// Fully restored TurbovecVectorStore with all collections.
pub async fn load(path: impl AsRef<Path>) -> Result<Self, VectorStoreError>
```
