# CLI Contracts: Integration API Tests

**Feature**: 015-integration-api-tests

## Contract 1: memory_test

### Command

```bash
cargo run --example memory_test [OPTIONS]
```

### Options

| Flag | Type | Env | Default | Required | Description |
|------|------|-----|---------|----------|-------------|
| `-k, --api-key` | `String` | `API_KEY` | — | Yes | DashScope API key |
| `-m, --model` | `String` | — | `qwen-plus` | No | Model name |
| `--keep-dir` | `bool` | — | `false` | No | Keep temp dir after run |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tests passed |
| 1 | At least one test failed or error |

### Output Contract

- First line: header with feature name and model
- Each test: `── <N>. <Test Name> ──` header, then result line
- Pass: `  ✓ <Test Name> (<duration>s)` + optional detail
- Fail: `  ✗ <Test Name> (<duration>s)` + error detail
- Final: `ALL <N> TESTS PASSED` or `<P> passed, <F> FAILED`
- API errors (invalid key): print diagnostic and exit 1

### Test Scenarios

1. **Write Memory**: Agent stores user preference → verify system prompt contains encoded memory
2. **Search Memory**: Query stored memory → agent references memory in response
3. **Memory Reasoning**: Multi-turn with memory context → agent uses memory to answer

---

## Contract 2: session_test

### Command

```bash
cargo run --example session_test [OPTIONS]
```

### Options

Same as `memory_test`.

### Output Contract

Same structure as `memory_test`.

### Test Scenarios

1. **Save/Load Roundtrip**: Create session with 2-turn conversation, save, load, verify message count
2. **Context Consistency**: Load saved session, ask about prior fact, verify answer references prior
3. **Close & Cleanup**: Close session, verify it can't be used, delete from store

---

## Contract 3: rag_test

### Command

```bash
cargo run --example rag_test [OPTIONS]
```

### Options

| Flag | Type | Env | Default | Required | Description |
|------|------|-----|---------|----------|-------------|
| `-k, --api-key` | `String` | `API_KEY` | — | Yes | DashScope API key |
| `-m, --model` | `String` | — | `qwen-plus` | No | Chat model name |
| `--embedding-model` | `String` | — | `text-embedding-v3` | No | Embedding model name |
| `--embedding-dims` | `u32` | — | `1536` | No | Embedding dimensions |

### Output Contract

Same structure as `memory_test`.

### Test Scenarios

1. **Ingest Document**: Index a document with known facts, verify chunks created
2. **Grounded Query**: Ask about indexed facts, verify answer contains facts from document
3. **Empty KB Query**: Ask question with empty KB, verify agent responds without errors

### Special Considerations

- Requires both chat API and embedding API to be available
- Embedding API may have separate rate limits — handle gracefully
- If embedding API fails, test 1 should fail with clear error

---

## Contract 4: streaming_tool_test

### Command

```bash
cargo run --example streaming_tool_test [OPTIONS]
```

### Options

Same as `memory_test`.

### Output Contract

Same structure as `memory_test`.

### Test Scenarios

1. **Single Tool Call**: Ask "3.14 * 2.718" → verify event counts (1 ToolCallStart/End, 1 ToolResultStart/End), verify answer ≈ 8.53452
2. **Multi-Tool Call**: Ask a two-step question → verify 2 ToolCall cycles

### Event Verification

- ToolCallStart count MUST equal ToolCallEnd count
- ToolResultStart count MUST equal ToolResultEnd count
- ToolCallStart event MUST precede corresponding ToolCallEnd
- At least one TextBlockDelta with non-empty delta text
- ReplyStart MUST appear before ReplyEnd
