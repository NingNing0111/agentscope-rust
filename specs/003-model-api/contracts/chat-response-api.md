# Contract: ChatResponse API

**Feature**: 003-model-api | **Version**: 0.1.0

## Struct: ChatResponse

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    pub content: Vec<ContentBlock>,   // TextBlock | ThinkingBlock | ToolCallBlock | DataBlock

    pub is_last: bool,

    #[serde(default = "generate_id")]
    pub id: String,

    #[serde(default = "generate_timestamp")]
    pub created_at: String,

    #[serde(rename = "type", default = "chat_response_type")]
    pub response_type: String,  // always "chat_response"

    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,

    #[serde(default)]
    pub finished_reason: FinishedReason,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, JsonValue>,
}
```

Where `ContentBlock` is imported from `agent_scope_message`:

```rust
pub enum ContentBlock {
    Text(TextBlock),
    Thinking(ThinkingBlock),
    ToolCall(ToolCallBlock),
    Data(DataBlock),
    // Hint and ToolResult blocks are NOT expected in ChatResponse
}
```

## Methods

### append_text

```rust
/// Append text to a TextBlock in content.
///
/// If `block_id` is provided and matches an existing TextBlock, appends to it.
/// Otherwise, creates a new TextBlock with the given text.
///
/// Returns `&mut Self` for chaining.
pub fn append_text(&mut self, text: &str, block_id: Option<&str>) -> &mut Self;
```

**Behavior**:
1. If `block_id` is `Some(id)`, search content for a TextBlock with that id → append `text` to `block.text`.
2. If no match or `block_id` is `None`, push a new TextBlock to content.
3. Returns self for method chaining.

### append_thinking

```rust
/// Append thinking content to a ThinkingBlock in content.
///
/// Similar to `append_text` but also merges provider-specific extras.
pub fn append_thinking(
    &mut self,
    thinking: &str,
    block_id: Option<&str>,
    extra_fields: HashMap<String, JsonValue>,
) -> &mut Self;
```

**Behavior**:
1. Match ThinkingBlock by id (or None→new).
2. Append `thinking` to the block's `thinking` string.
3. Merge `extra_fields` — non-None values overwrite existing fields on the block (via `#[serde(flatten)]`).
4. If new block, all extra_fields are attached.

### append_tool_call

```rust
/// Append tool call input to a ToolCallBlock in content.
///
/// `block_id` is REQUIRED — tool calls are always identified by id.
pub fn append_tool_call(
    &mut self,
    block_id: &str,
    name: &str,
    input: &str,
    extra_fields: HashMap<String, JsonValue>,
) -> &mut Self;
```

**Behavior**:
1. Match ToolCallBlock by `block_id` → append `input` to existing input string, merge extras.
2. No match → create new ToolCallBlock with given id/name/input, merge extras.

### append_data_block

```rust
/// Append raw media bytes to a DataBlock in content.
///
/// For `audio/*`: raw bytes are accumulated (decode→concat→re-encode base64).
/// For other media types: each delta replaces the previous DataBlock.
pub fn append_data_block(
    &mut self,
    block_id: &str,
    data: &[u8],
    media_type: &str,
    name: Option<&str>,
) -> &mut Self;
```

**Behavior (audio)**:
1. Match DataBlock by id, source is Base64Source, media_type matches.
2. Decode existing base64 → concat with new raw bytes → re-encode as base64.
3. Store back.

**Behavior (non-audio)**:
1. Match or create new.
2. Replace source with new Base64Source (base64-encode the raw bytes).

### append_chat_response

```rust
/// Merge another ChatResponse (delta chunk) into this one.
pub fn append_chat_response(&mut self, other: &ChatResponse) -> &mut Self;
```

**Behavior**:
1. Build a map of `block_id → index` for `other.content`.
2. For each block in `self.content`:
   - If its id matches a block in `other`, for matching block types:
     - TextBlock: concatenate `text`
     - ThinkingBlock: concatenate `thinking`, merge extras
     - ToolCallBlock: concatenate `input`, merge extras
     - DataBlock (audio/*): decode→concat bytes→re-encode
     - DataBlock (other): replace source with delta's source
   - Remove matched block from `other`'s map
3. Append remaining unmatched blocks from `other` to `self.content` (deep cloned).
4. If `other.usage` is `Some`, replace `self.usage`.

### get_text_content

```rust
/// Join all TextBlock text fields with the given separator.
pub fn get_text_content(&self, separator: &str) -> String;
```

## JSON serialization format

```json
{
  "content": [
    {
      "type": "text",
      "text": "Hello!",
      "id": "abc123",
      "created_at": "2026-07-28T10:00:00Z",
      "finished_at": null
    }
  ],
  "is_last": true,
  "id": "resp-xyz",
  "created_at": "2026-07-28T10:00:01Z",
  "type": "chat_response",
  "usage": null,
  "finished_reason": "completed",
  "metadata": {}
}
```

## Key invariants

1. `content` is ALWAYS `Vec<ContentBlock>` (not `Vec<TextBlock | ...>` — uses the tagged enum).
2. `is_last` distinguishes incremental chunks from the final accumulated response.
3. In streaming mode, each chunk has `is_last=false`; the StreamAccumulator-built final chunk has `is_last=true`.
4. ContentBlock.type tag MUST be one of: `"text"`, `"thinking"`, `"tool_call"`, `"data"`.
5. ToolCallBlock.input is a raw JSON string — NO runtime parsing.
6. DataBlock media types starting with `"audio/"` follow streaming accumulation semantics; all others use replace semantics.
