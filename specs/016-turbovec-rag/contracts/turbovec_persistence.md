# Contract: TurbovecVectorStore 持久化格式

**Feature**: 016-turbovec-rag
**Version**: 1

## Directory Structure

```text
{store_path}/
├── manifest.json           # Store-level metadata
├── {collection_name}.tvim  # turbovec IdMapIndex file (turbovec native binary format)
└── {collection_name}.meta  # Chunk metadata JSON
```

## manifest.json

```json
{
  "version": 1,
  "bit_width": 4,
  "collections": {
    "my_kb": {
      "dim": 1536,
      "n_vectors": 1050
    },
    "other_kb": {
      "dim": 768,
      "n_vectors": 500
    }
  }
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `version` | `u32` | Format version. Loader reject files with version > current. |
| `bit_width` | `usize` | Compression bits per coordinate (2/3/4), shared across all collections |
| `collections` | `Map<String, CollectionEntry>` | Collection name → metadata |
| `collections.{name}.dim` | `usize` | Vector dimensionality |
| `collections.{name}.n_vectors` | `usize` | Number of stored vectors (for integrity validation on load) |

### Validation on Load

- `version` must be 1 (reject unknown versions)
- `bit_width` must be 2, 3, or 4
- Each `{name}.tvim` file must exist for every collection entry
- Each `{name}.meta` file must exist for every collection entry
- `IdMapIndex::load("{name}.tvim")` must succeed
- Loaded `index.len()` must equal `n_vectors` (mismatch → corrupted)

## {collection_name}.tvim

turbovec 原生 `IdMapIndex` 持久化格式。由 `IdMapIndex::write(path)` 生成，`IdMapIndex::load(path)` 还原。

关键属性：
- 使用原子写入（temp → fsync → rename），断电安全
- 包含所有向量数据的压缩编码 + TQ+ calibration state
- 跨平台二进制兼容（相同 turbovec 版本）

## {collection_name}.meta

```json
{
  "chunks": {
    "1234567890123456": {
      "document_id": "doc-abc123",
      "chunk_index": 0,
      "total_chunks": 5,
      "source": "policy.md",
      "metadata": {
        "tenant_id": "t1",
        "category": "hr"
      }
    },
    "9876543210987654": {
      "document_id": "doc-abc123",
      "chunk_index": 1,
      "total_chunks": 5,
      "source": "policy.md",
      "metadata": {
        "tenant_id": "t1",
        "category": "hr"
      }
    }
  }
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `chunks` | `Map<String (u64), ChunkMetaEntry>` | Internal ID → chunk metadata |

### ChunkMetaEntry

| Field | Type | Description |
|-------|------|-------------|
| `document_id` | `String` | Owning document |
| `chunk_index` | `usize` | Zero-based position in document |
| `total_chunks` | `usize` | Total chunks for this document |
| `source` | `String` | Original source filename |
| `metadata` | `HashMap<String, String>` | Key-value metadata |

### Rebuilding doc_index on Load

`doc_index` (document_id → Vec<u64>) is NOT serialized directly. It is rebuilt from `chunks` during load:

```rust
for (id_str, meta) in manifest.chunks {
    let id: u64 = id_str.parse()?;
    doc_index.entry(meta.document_id.clone())
        .or_default()
        .push(id);
}
```

This is O(n) with a single pass over chunks — acceptable for a load operation.

## Forward Compatibility

- Loader must reject `version > CURRENT_VERSION` with an informative error
- Unknown optional fields in `manifest.json` are silently ignored
- Unknown fields in `chunks` entries (`{collection}.meta`) are silently ignored
