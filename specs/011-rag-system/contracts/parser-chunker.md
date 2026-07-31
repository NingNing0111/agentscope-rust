# Contract: Parser & Chunker

**Feature**: 011-rag-system
**Crate**: `agent_scope_rag`
**Date**: 2026-07-31

## Section Data Type

```rust
#[derive(Debug, Clone)]
pub enum SectionContent {
    Text(String),
    DataBlock(DataBlockData),
}

pub struct Section {
    pub content: SectionContent,
    pub source: String,
    pub metadata: HashMap<String, String>,
}
```

## Parser Trait

```rust
pub trait Parser: Send + Sync {
    /// Parse raw bytes into logical sections.
    /// - `file`: raw file content
    /// - `filename`: original filename (for source attribution)
    fn parse(&self, file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError>;
}
```

## TextParser

```rust
pub struct TextParser;

impl Parser for TextParser {
    fn parse(&self, file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError> {
        // Empty file → empty vec
        // Non-empty text → single Section with UTF-8 content
    }
}
```

## Chunk Data Type

```rust
pub struct Chunk {
    pub content: String,
    pub source: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub metadata: HashMap<String, String>,
}
```

## Chunker Trait

```rust
pub trait Chunker: Send + Sync {
    fn chunk(&self, sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>;
}
```

## ApproxTokenChunker

```rust
pub struct ApproxTokenChunker {
    pub chunk_size: usize,
    pub overlap: usize,
}
```

## Error Types

```rust
pub enum ParserError {
    /// File format not supported by this parser
    UnsupportedFormat { format: String, filename: String },
    /// UTF-8 decoding failed
    EncodingError { filename: String, error: String },
}

pub enum ChunkerError {
    /// chunk_size must be > overlap
    InvalidParameters { chunk_size: usize, overlap: usize },
}
```

## Behavioral Contract

### Parser

1. `parse(file, filename)` 返回 Section 列表：
   - 空文件（`file.is_empty()`）→ `Ok(vec![])`
   - 非空文本文件 → `Ok(vec![Section { content: SectionContent::Text(utf8_string), source: filename, metadata: {} }])`
   - `.md` 文件末尾被识别为文本，当前版本不按标题拆分
   - 文件生成不支持的格式 → `Err(ParserError::UnsupportedFormat)`

2. Section 不可合并约束：
   - 来自不同 source 的 Section 绝不合并
   - Chunker 不能跨 Section 边界
   - 单个 Section 可能被切分成多个 Chunk

### Chunker

1. `chunk(sections)` 返回 Chunk 列表：
   - 空 sections → `Ok(vec![])`（非错误）
   - 每个 Section 独立切分
   - 同 source 的所有 Chunk 的 `total_chunks` 相等
   - `chunk_index` 从 0 开始连续递增

2. 滑动窗口行为：
   - 窗口大小：`chunk_size` tokens
   - 步长：`chunk_size - overlap`
   - 最后一个 chunk 可能小于 `chunk_size`

3. Token 计数（启发式）：
   - 英文文本：按空格分词，1 word ≈ 1 token
   - 非英文（Unicode 非 ASCII 非空白字符）：4 字符 ≈ 1 token
   - 这近似于但不等于精确的 tokenizer 行为

4. 参数验证：
   - `chunk_size > overlap`（或者两者相等）
   - 如果 `chunk_size <= overlap` → `Err(ChunkerError::InvalidParameters)`

### Parser → Chunker 管道

```
   raw bytes + filename
         │
         ▼
    Parser::parse()
         │
         ▼
     Vec<Section>        ← 每个 Section 有不同的 source 或相同 source
         │
         ▼
    Chunker::chunk()
         │
         ▼
     Vec<Chunk>         ← 所有 Chunk 按 source 关联、index 排序
```
