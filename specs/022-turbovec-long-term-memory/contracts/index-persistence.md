# Contract: Index Persistence

**Feature**: 022-turbovec-long-term-memory
**Contract Type**: Persistence format specification
**Stability**: New (may evolve)
**Depends on**: `agent_scope_rag::TurbovecVectorStore` save/load format (Feature 016)

## Directory Layout

```text
{memory_dir}/.turbovec/
├── manifest.json          # StoreManifest version + collection metadata
├── memories.tvim          # turbovec IdMapIndex binary file
└── memories.meta          # JSON: chunk metadata + document reverse index
```

`{memory_dir}` is resolved from `TurbovecMemoryConfig::memory_dir`.

## manifest.json

```json
{
  "version": 1,
  "bit_width": 4,
  "collections": {
    "memories": {
      "dim": 1536,
      "n_vectors": 42
    }
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `version` | `u32` | Schema version (current: 1) |
| `bit_width` | `usize` | TurboVec compression level (2/3/4) |
| `collections` | `Map<String, CollectionManifestEntry>` | One entry per collection |

### CollectionManifestEntry

| Field | Type | Description |
|-------|------|-------------|
| `dim` | `usize` | Vector dimension (e.g., 1536) |
| `n_vectors` | `usize` | Number of indexed vectors |

## {collection}.meta

JSON file containing chunk metadata and document reverse index. Keyed by internal u64 ID (stringified).

```json
{
  "chunks": {
    "1": {
      "document_id": "user-role",
      "chunk_index": 0,
      "total_chunks": 1,
      "source": "user-role.md",
      "content": "The user works as a senior data scientist...",
      "metadata": {
        "memory_name": "user-role",
        "memory_type": "user",
        "source": "user-role.md",
        "updated_at": "2026-08-02T12:00:00+00:00"
      }
    }
  }
}
```

## Load Validation

When `TurbovecMemory::new()` loads an existing index:

1. Read `manifest.json` → verify `version <= CURRENT_VERSION`
   - `version > CURRENT_VERSION`: return `SemanticIndexError` ("unsupported index version")
2. Verify `bit_width ∈ {2, 3, 4}`: return `SemanticIndexError` if invalid
3. For each collection in manifest:
   - Load `.tvim` file → verify `index.len() == entry.n_vectors`
   - Mismatch: return `SemanticIndexError` ("corrupted: vector count mismatch")
4. Load `.meta` file → rebuild `doc_index` from chunk metadata
   - Missing `.meta`: return `SemanticIndexError` ("corrupted: metadata file missing")
5. Verify embedding model dimensions match collection `dim`
   - Mismatch + `auto_rebuild == true`: trigger rebuild
   - Mismatch + `auto_rebuild == false`: return `SemanticIndexError` ("dimension mismatch: rebuild needed")

## Save Behavior

- `save_index()` delegates to `TurbovecVectorStore::save(path)`
- Directory `{memory_dir}/.turbovec/` is created if it doesn't exist
- Writes are NOT atomic across all files — individual file writes use temp→rename where supported by the OS
- Partial writes (crash during save): index may be corrupted; caller should `rebuild_index()` to recover

## Forward Compatibility

- Unknown JSON keys in manifest and meta files are ignored (serde `#[serde(deny_unknown_fields)]` is NOT used)
- Unknown collection entries in manifest are loaded normally
- New optional fields with defaults are safe to add in future versions

## Migration

- `version` bump to 2+: implement a migration path or require rebuild
- Collection dimension change: requires `rebuild_index()` (no in-place migration)
- `bit_width` change: requires `rebuild_index()` (no in-place migration)
