//! Msg/ContentBlock ↔ rig `Message`/`AssistantContent` 双向映射。
//!
//! **T005 出站**：[`msg_to_rig_messages`] 把 agent_scope 消息转换为 rig 消息。
//! **T006 入站**：[`assistant_content_to_blocks`] 把 rig assistant 内容归一化为
//! `ContentBlock` 列表（非流式补全用）。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §1。
//! 已记录偏差（rig 0.41.0 相对契约假设的 0.42）：
//! - 契约 `Message::ToolResult` 独立变体 → 0.41 实为 `Message::User{UserContent::ToolResult}`。
//! - 契约 `Assistant.thinking` 字段 → 0.41 用 `AssistantContent::Reasoning`。
//! - 契约 `ModelError::EmptyResponse` 变体不存在 → 空 choice 用 `FormatError` 替代。

// message 子模块类型不在 `rig::completion` 顶层 re-export（mod.rs 仅
// `pub use message::{AssistantContent, Message, MessageError}`），需显式路径。
use rig::completion::message::{
    DocumentSourceKind, Image, ImageMediaType, MimeType, ToolResult, ToolResultContent, UserContent,
};
use rig::completion::{AssistantContent, Message};
use rig::core::OneOrMany;

use agent_scope_message::block::{
    ContentBlock, DataBlock, DataSource, HintBlock, HintBlockItem, HintContent, TextBlock,
    ThinkingBlock, ToolCallBlock, ToolOutput, ToolResultBlock, ToolResultBlockItem,
};
use agent_scope_message::msg::{Msg, Role};
use agent_scope_message::{Base64Source, URLSource};
use agent_scope_model::FormatError;
use agent_scope_model::model_error::ModelError;

// ---------------------------------------------------------------------------
// 出站：agent_scope → rig
// ---------------------------------------------------------------------------

/// 把 agent_scope 消息列表转换为 rig 消息列表（T005）。
///
/// 规则（契约 §1.1）：
/// - `System` → 一条 `Message::System`（全部文本块以换行拼接）。
/// - `User` → 主 `Message::User`（Text→Text、Data→Image），随后每个
///   `ToolResultBlock` 展开为一条独立的 `Message::User{UserContent::ToolResult}`。
/// - `Assistant` → 一条 `Message::Assistant`（Text→Text、Thinking→`Reasoning`、
///   ToolCall→ToolCall、Data→Image），其中的 `ToolResultBlock` 同样展开为独立的
///   `Message::User{UserContent::ToolResult}`（引擎以 Assistant role 存储工具结果）。
/// - `Hint` 块不发送改为以文本内容发送（注入给模型看的参考/指令，如 RAG 检索知识、记忆、任务提醒；注入方负责在文本内声明 untrusted 边界）；`Unknown` 块 → `FormatError`。
/// - 主内容为空时跳过该消息。
pub fn msg_to_rig_messages(msgs: &[Msg]) -> Result<Vec<Message>, ModelError> {
    let mut out = Vec::new();
    for msg in msgs {
        match msg.role {
            Role::System => {
                let text = join_text_blocks(&msg.content);
                if !text.is_empty() {
                    out.push(Message::system(text));
                }
            }
            Role::User => {
                // 主 user 内容（Text/Data）。
                let mut primary: Vec<UserContent> = Vec::new();
                // ToolResult 块展开为独立消息，紧随主消息之后（契约 §1.1）。
                let mut tool_results: Vec<ToolResult> = Vec::new();
                for block in &msg.content {
                    match block {
                        ContentBlock::Text(tb) => {
                            primary.push(UserContent::text(tb.text.clone()));
                        }
                        ContentBlock::Data(db) => {
                            primary.push(UserContent::Image(block_data_to_rig_image(db)));
                        }
                        ContentBlock::Hint(hb) => {
                            primary.push(UserContent::text(hint_text(hb)));
                        }
                        ContentBlock::ToolResult(tr) => {
                            tool_results.push(block_to_rig_tool_result(tr));
                        }
                        other => return Err(unsupported_block(other)),
                    }
                }
                if let Some(content) = OneOrMany::from_iter_optional(primary) {
                    out.push(Message::User { content });
                }
                for tr in tool_results {
                    out.push(Message::User {
                        content: OneOrMany::one(UserContent::ToolResult(tr)),
                    });
                }
            }
            Role::Assistant => {
                let mut contents: Vec<AssistantContent> = Vec::new();
                // 引擎侧工具结果以 `Role::Assistant` 存储（add_tool_result_to_context），
                // 此处同样展开为独立 ToolResult 消息（契约 §1.1：ToolResult 展开
                // 为独立消息；wire 端 role=tool，rig 以 UserContent::ToolResult 表达）。
                let mut tool_results: Vec<ToolResult> = Vec::new();
                for block in &msg.content {
                    match block {
                        ContentBlock::Text(tb) => {
                            contents.push(AssistantContent::text(tb.text.clone()));
                        }
                        ContentBlock::Thinking(tb) => {
                            // 契约 `Assistant.thinking` 字段在 0.41 不存在，
                            // 用 `AssistantContent::Reasoning` 表达（已记录偏差）。
                            contents.push(AssistantContent::reasoning(&tb.thinking));
                        }
                        ContentBlock::ToolCall(tc) => {
                            let args = parse_tool_call_input(&tc.input)?;
                            contents.push(AssistantContent::tool_call(
                                tc.id.clone(),
                                tc.name.clone(),
                                args,
                            ));
                        }
                        ContentBlock::Data(db) => {
                            contents.push(AssistantContent::Image(block_data_to_rig_image(db)));
                        }
                        ContentBlock::ToolResult(tr) => {
                            tool_results.push(block_to_rig_tool_result(tr));
                        }
                        ContentBlock::Hint(hb) => {
                            contents.push(AssistantContent::text(hint_text(hb)));
                        }
                        other => return Err(unsupported_block(other)),
                    }
                }
                if let Some(content) = OneOrMany::from_iter_optional(contents) {
                    out.push(Message::Assistant { id: None, content });
                }
                for tr in tool_results {
                    out.push(Message::User {
                        content: OneOrMany::one(UserContent::ToolResult(tr)),
                    });
                }
            }
        }
    }
    Ok(out)
}

/// 拼接全部文本块（System 消息用）。
fn join_text_blocks(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text(tb) => Some(tb.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 提取 `HintBlock` 的文本内容：`Text` 直接取，`Blocks` 拼接各文本项
/// （数据项跳过）。Hint 是注入给模型看的参考/指令（RAG 知识、记忆、任务
/// 提醒等），发送时以纯文本表达。
fn hint_text(hb: &HintBlock) -> String {
    match &hb.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(items) => items
            .iter()
            .filter_map(|i| match i {
                HintBlockItem::Text(t) => Some(t.text.clone()),
                HintBlockItem::Data(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// `ToolCallBlock.input`（JSON 字符串）→ `serde_json::Value`。
///
/// 空/空白输入按空对象处理（OpenAI 兼容请求中 `arguments: "{}"` 常见）；
/// 非空且解析失败 → `FormatError`（契约 §1.1）。
fn parse_tool_call_input(input: &str) -> Result<serde_json::Value, ModelError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(trimmed).map_err(|e| ModelError::FormatError {
        context: "rig:tool-call".to_string(),
        source: FormatError::InvalidMessage(format!("invalid tool call arguments JSON: {e}")),
    })
}

/// `ToolResultBlock` → rig `ToolResult`（id 沿用块 id，call_id 不设）。
fn block_to_rig_tool_result(tr: &ToolResultBlock) -> ToolResult {
    let content = match &tr.output {
        ToolOutput::Text(s) => OneOrMany::one(ToolResultContent::text(s.clone())),
        ToolOutput::Blocks(items) => {
            let mut v: Vec<ToolResultContent> = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    ToolResultBlockItem::Text(tb) => {
                        v.push(ToolResultContent::text(tb.text.clone()));
                    }
                    ToolResultBlockItem::Data(db) => {
                        v.push(ToolResultContent::Image(block_data_to_rig_image(db)));
                    }
                }
            }
            // rig 的 OneOrMany 不能为空：空输出以空文本占位。
            OneOrMany::from_iter_optional(v)
                .unwrap_or_else(|| OneOrMany::one(ToolResultContent::text(String::new())))
        }
    };
    ToolResult {
        id: tr.id.clone(),
        call_id: None,
        content,
    }
}

/// `DataBlock` → rig `Image`（Base64/Url 源，media_type 用 `MimeType::from_mime_type` 解析）。
fn block_data_to_rig_image(db: &DataBlock) -> Image {
    match &db.source {
        DataSource::Base64(Base64Source { data, media_type }) => Image {
            data: DocumentSourceKind::base64(data),
            media_type: ImageMediaType::from_mime_type(media_type),
            detail: None,
            additional_params: None,
        },
        DataSource::Url(URLSource { url, media_type }) => Image {
            data: DocumentSourceKind::url(url),
            media_type: ImageMediaType::from_mime_type(media_type),
            detail: None,
            additional_params: None,
        },
    }
}

/// 不支持的 content block → `FormatError`。
fn unsupported_block(b: &ContentBlock) -> ModelError {
    ModelError::FormatError {
        context: "rig:message".to_string(),
        source: FormatError::InvalidMessage(format!("unsupported content block: {b:?}")),
    }
}

// ---------------------------------------------------------------------------
// 入站：rig → agent_scope
// ---------------------------------------------------------------------------

/// 把 rig assistant 内容归一化为 `ContentBlock` 列表（T006，非流式）。
///
/// 规则（契约 §1.2）：
/// - `Text` → `TextBlock`
/// - `Reasoning` → `ThinkingBlock`（`display_text()`；`id` 记入 `extras["reasoning_id"]`）
/// - `ToolCall` → `ToolCallBlock`（`arguments` 序列化为 JSON 字符串）
/// - `Image` → `DataBlock`（仅 Url/Base64 源可表达，其余源 → `FormatError`）
///
/// 空 `choice` → `ModelError::FormatError`（契约 §1.2 记 `EmptyResponse`，
/// 框架无此变体，已记录偏差）。
pub fn assistant_content_to_blocks(
    choice: Vec<AssistantContent>,
) -> Result<Vec<ContentBlock>, ModelError> {
    if choice.is_empty() {
        return Err(ModelError::FormatError {
            context: "rig:response".to_string(),
            source: FormatError::InvalidMessage("empty assistant response".to_string()),
        });
    }
    let mut blocks = Vec::with_capacity(choice.len());
    for item in choice {
        match item {
            AssistantContent::Text(t) => blocks.push(ContentBlock::Text(TextBlock::new(t.text))),
            AssistantContent::Reasoning(r) => {
                let mut tb = ThinkingBlock::new(r.display_text());
                if let Some(id) = r.id {
                    tb.extras
                        .insert("reasoning_id".to_string(), serde_json::Value::String(id));
                }
                blocks.push(ContentBlock::Thinking(tb));
            }
            AssistantContent::ToolCall(tc) => {
                let input = serde_json::to_string(&tc.function.arguments).map_err(|e| {
                    ModelError::SerializationError {
                        context: "rig:tool-call".to_string(),
                        source: e,
                    }
                })?;
                blocks.push(ContentBlock::ToolCall(ToolCallBlock::new(
                    tc.id,
                    tc.function.name,
                    input,
                )));
            }
            AssistantContent::Image(img) => {
                blocks.push(ContentBlock::Data(rig_image_to_block_data(img)?));
            }
        }
    }
    Ok(blocks)
}

/// rig `Image` → `DataBlock`（仅 Url/Base64 源可表达）。
fn rig_image_to_block_data(img: Image) -> Result<DataBlock, ModelError> {
    let media_type = img
        .media_type
        .map(|m| m.to_mime_type().to_string())
        .unwrap_or_default();
    let source = match img.data {
        DocumentSourceKind::Url(url) => DataSource::Url(URLSource { url, media_type }),
        DocumentSourceKind::Base64(data) => DataSource::Base64(Base64Source { data, media_type }),
        other => {
            return Err(ModelError::FormatError {
                context: "rig:data".to_string(),
                source: FormatError::InvalidMessage(format!(
                    "unsupported image data source: {other:?}"
                )),
            });
        }
    };
    Ok(DataBlock::new(source))
}
