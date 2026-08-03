# 参考:消息与基础类型(`agent_scope_message` / `agent_scope_types`)

> 详细 API 参考:`Msg`、`ContentBlock`、消息工厂函数、工具状态机、序列化协议与共享错误类型。

## 1. `Msg` 结构

`Msg` 是所有模块交换消息的核心类型。

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 发送者 / Agent 名称 |
| `content` | `Vec<ContentBlock>` | 消息内容块列表 |
| `role` | `Role` | `User` / `Assistant` / `System`(序列化为小写) |
| `id` | `String` | 唯一标识,缺省自动生成 UUID |
| `metadata` | `serde_json::Value` | 任意元数据,缺省空 object |
| `created_at` | `String` | RFC 3339 时间戳,缺省自动生成 |
| `usage` | `Option<Usage>` | token 用量(`input_tokens`/`output_tokens`) |
| `finished_at` | `Option<String>` | 定稿时间 |
| `finished_reason` | `Option<ReplyFinishedReason>` | `completed`/`interrupted`/`exceed_max_iters`/`error` |
| `structured_output` | `Option<serde_json::Value>` | 结构化输出结果 |
| `error` | `Option<ErrorInfo>` | 消息表示失败时的错误信息 |

所有 `Option` 为 `None` 时序列化不输出该字段。

## 2. `ContentBlock`(6 种块类型)

带 `type` 判别标签的枚举(serde tagged union):

| 变体 | 载荷 | 职责 |
|------|------|------|
| `Text` | `TextBlock { text, id, created_at, finished_at }` | 纯文本 |
| `Thinking` | `ThinkingBlock { thinking, ..., extras }` | 模型推理内容;`extras` 扁平透传 Provider 私有字段 |
| `Hint` | `HintBlock { hint: HintContent, source, ... }` | 一次性、非流式提示/指令 |
| `Data` | `DataBlock { source: DataSource, name, ... }` | 二进制数据(`Base64Source` / `URLSource`) |
| `ToolCall` | `ToolCallBlock { id, name, input, state, suggested_rules, ... }` | 工具调用请求;`input` 为**未解析的原始 JSON 字符串** |
| `ToolResult` | `ToolResultBlock { id, name, output, state, metadata, is_last, ... }` | 工具执行结果;`output` 为字符串或块列表(untagged) |

运行时判别用 `block.block_type()` 返回 `BlockType` 枚举。

## 3. 消息工厂函数

```rust
use agent_scope_message::factory::{user_msg, assistant_msg, system_msg};

let user = user_msg("user", "你好")?;           // Result<Msg, ValidationError>
let sys  = system_msg("system", "你是助手")?;    // Result<Msg, ValidationError>
let asst = assistant_msg("assistant", "你好!");  // 直接返回 Msg(不返回 Result)
```

行为差异:

- `user_msg` / `system_msg`:校验角色-内容(仅 Text 等),成功时 `finished_at` 设为 `created_at`(创建即定稿)。
- `assistant_msg`:Assistant 接受全部块类型,校验必然通过,内部 `expect`;`finished_at` 为 `None`(随流式增量构建)。
- 带自定义内容块的变体:`user_msg_with_blocks` / `assistant_msg_with_blocks` / `system_msg_with_blocks`。

## 4. 角色校验规则

`Msg::new(name, content, role)` 构造时校验,违规返回 `ValidationError::InvalidContentForRole`:

| 角色 | 允许的块类型 |
|------|-------------|
| `User` | 仅 `Text`、`Data` |
| `System` | 仅 `Text` |
| `Assistant` | 全部 |

## 5. 工具状态机

- `ToolCallState`:`pending` → `asking` → `allowed` → `submitted` → `finished`
- `ToolResultState`:`running` → `success` / `error` / `interrupted` / `denied`

## 6. 常用读取方法

```rust
// 按类型过滤;Some(type) 只返回该类型,None 返回全部
let texts = msg.get_content_blocks(Some(BlockType::Text));
let has_tc = msg.has_content_blocks(Some(BlockType::ToolCall));
let is_empty = !msg.has_content_blocks(None);

// 拼接全部 TextBlock
if let Some(text) = msg.get_text_content("\n") { println!("{text}"); }
```

## 7. 序列化协议要点

1. 类型标签:`text`/`thinking`/`hint`/`data`/`tool_call`/`tool_result`(小写);`DataSource` 为 `base64`/`url`。
2. **前向兼容**:未知块类型反序列化为 `ContentBlock::Unknown`,不报错;但该变体**不保留原始字段**(有损)。
3. 缺省自动补全:`id`/`created_at`/`metadata`。
4. `Role` 与状态机小写;`ErrorType`/`ReplyFinishedReason` 为 snake_case。

## 8. 共享错误类型(`agent_scope_types`)

| 类型 | 说明 |
|------|------|
| `ErrorType` | `authentication`/`permission`/`rate_limit`/`invalid_request`/`upstream`/`connection`/`internal`/`unknown` |
| `ErrorInfo { error_type, message }` | 面向 UI 的结构化错误;序列化时字段名为 `type` |
| `ReplyFinishedReason` | `completed`/`interrupted`/`exceed_max_iters`/`error` |

## 9. 错误

| 错误 | 触发条件 |
|------|----------|
| `ValidationError::InvalidContentForRole` | 内容块违反角色校验 |
| `ValidationError::EmptyContent` | 内容为空(可选检查) |
| `AppendEventError::ReplyIdMismatch` | 事件流 `reply_id` 与消息 `id` 不匹配 |
| `AppendEventError::BlockNotFound` | Delta/End 事件引用不存在的块 |
| `AppendEventError::UnknownEventType` | 无法识别的事件类型 |
