# Contract: Rebuild and Consistency

**Feature**: 022-turbovec-long-term-memory
**Contract Type**: Operational contract
**Stability**: New (may evolve)
**Depends on**: `contracts/turbovec-memory.md`, `contracts/semantic-index.md`, `contracts/index-persistence.md`

## Rebuild Workflow

### Trigger Conditions

`rebuild_index()` may be called:
1. **Explicitly** — developer calls `memory.rebuild_index().await`
2. **Automatically** — on `TurbovecMemory::new()` when:
   - `.turbovec/` directory is missing AND `config.auto_rebuild == true`
   - `manifest.json` version is unsupported AND `config.auto_rebuild == true`
   - Collection dimension differs from embedding model AND `config.auto_rebuild == true`
3. **Implicitly** — on `semantic_search()` when `auto_rebuild == true` and index is missing

### Rebuild Steps

```
Phase 1: Scan
  ├── FileMemory::list() → all .md files
  ├── Filter out MEMORY.md
  └── Collect: Vec<(name, path)>

Phase 2: Index
  For each (name, path):
  ├── FileMemory::read(name) → MemoryEntry?
  │   ├── Parse error → skip, increment skipped
  │   └── Empty content → skip, increment skipped
  ├── EmbeddingModel::embed(content) → Vec<f32>
  │   └── Embed error → record error, continue
  └── Build VectorRecord → accumulate

Phase 3: Replace
  ├── Drop old collection from TurbovecVectorStore
  ├── Create new collection with embedding_model.card().dimensions
  ├── Insert all accumulated VectorRecords
  └── save_index() → persist to .turbovec/

Phase 4: Report
  └── Return MemoryRebuildReport { total_scanned, indexed, skipped, errors, duration_ms }
```

### Idempotency

- Multiple consecutive `rebuild_index()` calls produce identical results (assuming no concurrent file changes)
- No duplicate vectors in collection after rebuild

### Partial Failure

- If a single file fails to parse/embed: skipped and counted; rebuild continues
- If ALL files fail: collection is empty; returns report with indexed=0
- If `save_index()` fails: rebuild is not persisted; caller should retry
- If process crashes during Phase 3: `.turbovec/` may be incomplete; next `rebuild_index()` fixes it

## Consistency Detection

### Stale Index Detection

The system does NOT automatically detect externally modified Markdown files. Consistency is checked at these points:

| Event | Detection | Action |
|-------|-----------|--------|
| `write()` | N/A (index is updated synchronously) | Always consistent |
| `delete()` | N/A (index is updated synchronously) | Always consistent |
| External file edit | NOT detected | Caller must `rebuild_index()` |
| External file deletion | NOT detected | Orphan vector remains (harmless) |
| `.turbovec/` missing | Detected on `new()` or `semantic_search()` | Error or auto-rebuild |
| Corrupted `.tvim` | Detected on load | Error |
| Dimension mismatch | Detected on load | Error or auto-rebuild |

### Orphan Vector Handling

Orphan = vector record exists in TurboVec but corresponding `.md` file is missing.

- Search may return orphan results → content is in vector metadata, not read from file
- Rebuild eliminates orphans (scans files, not index)
- Diagnostic: `list()` (file-based) vs `semantic_search()` (index-based) discrepancy indicates orphans

### Duplicate Vector Handling

Duplicate = two vector records with same `document_id`.

- Normal `write()` deletes old before insert → no duplicates
- Crash during write may leave duplicate → `rebuild_index()` eliminates
- External tooling that copies files may introduce duplicates → `rebuild_index()` eliminates

## Concurrent Access

- `TurbovecVectorStore` uses `tokio::sync::RwLock` — multiple readers, exclusive writer
- `write()`: acquires write lock for collection mutation (brief, synchronous)
- `semantic_search()`: acquires read lock (non-blocking except during write)
- `rebuild_index()`: acquires write lock for the entire rebuild duration
- No distributed locking — single-process only (per spec assumptions)
