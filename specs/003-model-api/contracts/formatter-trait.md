# Contract: Formatter Trait

**Feature**: 003-model-api | **Version**: 0.1.0

## Trait Definition

```rust
use agent_scope_message::{Msg, ContentBlock, TextBlock, DataBlock};

/// Converts AgentScope Msg objects into the format required by a specific
/// provider's API.
pub trait Formatter: Send + Sync {
    /// The media-type patterns this formatter supports as input
    /// (derived from `input_types`, excluding `text/plain` and
    /// `application/x-thinking`).
    fn supported_input_media_types(&self) -> &[String];

    /// Format a list of Msg objects into the provider API's message dicts.
    ///
    /// Each dict in the returned Vec has at minimum a `"role"` key and a
    /// `"content"` key whose structure is provider-specific.
    async fn format(
        &self,
        msgs: &[Msg],
    ) -> Result<Vec<serde_json::Value>, FormatError>;

    /// Separate the multimodal data embedded in a tool result's output.
    ///
    /// Returns:
    /// - `text`: A string representation for the LLM context (with
    ///   `<system-reminder>` wrappers for promoted multimodal blocks).
    /// - `blocks`: Multimodal blocks to be promoted to the user content
    ///   (TextBlock identifiers + DataBlocks for supported media types).
    fn convert_tool_result_to_string(
        &self,
        output: &ToolOutputType,     // String | Vec<ContentBlock>
    ) -> Result<(String, Vec<ContentBlock>), FormatError>;

    /// Group consecutive messages into tool sequences and agent messages,
    /// preserving the original order.
    fn group_messages(
        &self,
        msgs: &[Msg],
    ) -> Vec<(MessageGroup, Vec<&Msg>)>;
}

/// Group classification for message sequences
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageGroup {
    /// Consecutive tool_call/tool_result messages
    ToolSequence,
    /// Non-tool messages (all other roles/content)
    AgentMessage,
}
```

## Tool Result Conversion Logic

### Input

The `output` field of a `ToolResultBlock` is a union type:

```rust
pub enum ToolOutputType {
    Text(String),
    Blocks(Vec<ContentBlock>),  // TextBlock | DataBlock
}
```

### Output: `(String, Vec<ContentBlock>)`

| Portion | Content | Purpose |
|---------|---------|---------|
| `String` (text) | `<system-reminder>A(n) image file is returned and will be presented to you with the identifier [ABC123].</system-reminder>` | Injected into tool result text for LLM context |
| `Vec<ContentBlock>` (promoted) | `[TextBlock("- ABC123 (image file): "), DataBlock(image)]` | Appended to user message as multimodal input |

### Decision tree per block

```
For each block in output:
├── TextBlock → append text to string output
└── DataBlock:
    ├── media_type matches supported_input_media_types?
    │   YES → generate shortuuid identifier
    │          append "<system-reminder>...</system-reminder>" to string
    │          push (identifier TextBlock + DataBlock) to promoted blocks
    │
    ├── source is URLSource?
    │   YES → append "<system-reminder>...URL: {url}</system-reminder>" to string
    │
    └── source is Base64Source? (unsupported media type)
        YES → decode base64, save to temp file
              append "<system-reminder>...saved at: {path}</system-reminder>" to string
```

## Message Grouping Logic

```
group_type = None
for msg in msgs:
    if group_type is None:
        if msg has tool_call or tool_result blocks:
            group_type = ToolSequence
        else:
            group_type = AgentMessage
        group = [msg]
    elif group_type == ToolSequence:
        if msg has tool_call or tool_result blocks:
            group.append(msg)
        else:
            yield (ToolSequence, group)
            group = [msg]
            group_type = AgentMessage
    else:  # AgentMessage
        if msg has tool_call or tool_result blocks:
            yield (AgentMessage, group)
            group = [msg]
            group_type = ToolSequence
        else:
            group.append(msg)

if group:
    yield (group_type, group)
```

## Concrete Implementations

### OpenAIChatFormatter

```rust
pub struct OpenAIChatFormatter {
    pub input_types: Vec<String>,
}
```

The `format()` output for OpenAI Chat Completions:

```json
[
  {
    "role": "system",
    "content": "You are a helpful assistant."
  },
  {
    "role": "user",
    "content": [
      {"type": "text", "text": "Describe this image:"},
      {"type": "image_url", "image_url": {"url": "data:image/png;base64,..."}}
    ]
  },
  {
    "role": "assistant",
    "content": null,
    "tool_calls": [
      {
        "id": "call_123",
        "type": "function",
        "function": {
          "name": "get_weather",
          "arguments": "{\"city\":\"Beijing\"}"
        }
      }
    ]
  }
]
```

## FormatError

```rust
pub enum FormatError {
    /// A message has an unsupported structure
    InvalidMessage(String),
    /// Media type not supported by this formatter
    UnsupportedMediaType { media_type: String, block_id: String },
    /// I/O error (temp file creation for base64 data)
    Io(std::io::Error),
    /// Base64 decode failure
    Base64Decode(base64::DecodeError),
}
```

## Invariants

1. `format()` output is always a `Vec<serde_json::Value>` (not strongly typed) — each provider has its own format.
2. `format()` is async to support any async operations in formatter logic.
3. `convert_tool_result_to_string` is called during `format()` when processing tool result blocks.
4. The `<system-reminder>` markers in the text output are NOT part of an AgentScope protocol — they are hints for the LLM to understand the context.
5. Promoted multimodal blocks replace the tool result's output blocks in the final API message.
6. `group_messages` must preserve original message ordering within groups.
