# 消息与基础类型 / Message & Basic Types

> 一句话定位：AgentScope 全部通信的统一数据协议——`Msg` 消息结构与 `ContentBlock` 内容块体系，是所有模块（Agent、模型、记忆、会话、RAG）的数据交换基础。

## 1. 模块概述 (Overview)

本模块对应两个基础 crate，处于分层架构最底层，不依赖任何其他 AgentScope crate：

| Crate | 职责 |
|-------|------|
| `agent_scope_message` | 消息结构 `Msg`、内容块 `ContentBlock` 体系、数据源、工具调用状态机、消息工厂函数 |
| `agent_scope_types` | 跨 crate 共享的通用类型：错误分类（`ErrorType`/`ErrorInfo`）、回复结束原因（`ReplyFinishedReason`）、Hook 类型标记、JSON 类型别名 |

**适用场景**：任何需要构造、读取、过滤或序列化消息的场景——向 Agent 发送用户输入、解析模型回复、持久化会话记录、自定义中间件中检查消息内容。

**前置阅读**：无（本模块是其余所有模块文档的前置）。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 消息结构 `Msg`

`Msg` 是 AgentScope 的核心消息类型，由发送者名、角色和一组内容块组成：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 发送者 / Agent 名称 |
| `content` | `Vec<ContentBlock>` | 组成消息体的内容块列表 |
| `role` | `Role` | 角色判别：`User` / `Assistant` / `System`（序列化为小写） |
| `id` | `String` | 唯一标识；构造或反序列化缺省时自动生成 UUID |
| `metadata` | `serde_json::Value` | 任意元数据字典，缺省为空 object |
| `created_at` | `String` | RFC 3339 创建时间戳，缺省自动生成 |
| `usage` | `Option<Usage>` | Token 用量（`input_tokens`/`output_tokens`），模型调用完成后写入 |
| `finished_at` | `Option<String>` | 消息定稿时间 |
| `finished_reason` | `Option<ReplyFinishedReason>` | 回复结束原因：`completed`/`interrupted`/`exceed_max_iters`/`error` |
| `structured_output` | `Option<serde_json::Value>` | 结构化输出（如 JSON mode / function calling 结果） |
| `error` | `Option<ErrorInfo>` | 消息表示失败时的结构化错误信息 |

所有 `Option` 字段为 `None` 时序列化输出中不出现（`skip_serializing_if`）。

### 2.2 内容块 `ContentBlock`

`ContentBlock` 是带 `type` 判别标签的枚举（serde tagged union），共 6 种块类型：

| 变体（`type` 标签） | 载荷类型 | 职责 |
|---------------------|----------|------|
| `"text"` | `TextBlock { text, id, created_at, finished_at }` | 纯文本 |
| `"thinking"` | `ThinkingBlock { thinking, ..., extras }` | 模型推理内容；`extras` 以 `#[serde(flatten)]` 透传 Provider 私有字段（如签名） |
| `"hint"` | `HintBlock { hint: HintContent, source, ... }` | 一次性、非流式的提示/指令内容；`HintContent` 为字符串或块列表（untagged） |
| `"data"` | `DataBlock { source: DataSource, name, ... }` | 二进制数据；`DataSource` 为 `"base64"`（`Base64Source{data, media_type}`）或 `"url"`（`URLSource{url, media_type}`） |
| `"tool_call"` | `ToolCallBlock { id, name, input, state, suggested_rules, ... }` | 工具调用请求；`input` 为**未解析的原始 JSON 字符串**（Foundation 层不解析参数） |
| `"tool_result"` | `ToolResultBlock { id, name, output, state, metadata, is_last, ... }` | 工具执行结果；`output` 为字符串或块列表（untagged `ToolOutput`）；`is_last` 标记流式工具结果的最后一块 |

运行时判别可用 `block_type()` 方法返回 `BlockType`（`Text`/`Thinking`/`Hint`/`Data`/`ToolCall`/`ToolResult`）。

### 2.3 角色与校验规则

`Msg::new(name, content, role)` 在构造时执行角色-内容校验，违反规则返回 `ValidationError::InvalidContentForRole`：

| 角色 | 允许的块类型 |
|------|-------------|
| `Role::User` | 仅 `Text`、`Data` |
| `Role::System` | 仅 `Text` |
| `Role::Assistant` | 全部块类型（无限制） |

### 2.4 工具调用状态机

两个状态枚举随块一同序列化（小写值）：

- `ToolCallState`：`pending`（默认）→ `asking` → `allowed` → `submitted` → `finished`
- `ToolResultState`：`running`（默认）→ `success` / `error` / `interrupted` / `denied`

`PermissionRule` 当前为占位结构（`extras` 扁平透传任意字段），将由权限模块完善。

### 2.5 序列化协议

用户可观察的序列化语义（宪法第十二条：稳定数据协议）：

1. **类型标签**：`ContentBlock` 与 `DataSource` 均以 `type` 字段判别，块标签为 `text`/`thinking`/`hint`/`data`/`tool_call`/`tool_result`。
2. **前向兼容**：反序列化遇到未知块类型时不报错，吸收为 `ContentBlock::Unknown` 占位变体；其 `block_type()` 尽力回退为 `Text`。注意：该变体不保留原始字段，重新序列化会丢失原始内容（有损）。
3. **缺省自动补全**：`id` 缺省时自动生成 UUID；`created_at` 缺省时自动生成 RFC 3339 时间戳；`metadata` 缺省为空 object。
4. **可选字段省略**：`usage`/`finished_at`/`finished_reason`/`structured_output`/`error` 为 `None` 时不出现在 JSON 中。
5. **枚举值风格**：`Role` 与状态机为小写（`"user"`/`"pending"`）；`ErrorType` 与 `ReplyFinishedReason` 为 snake_case（`"rate_limit"`/`"exceed_max_iters"`）。

### 2.6 共享错误类型（`agent_scope_types`）

| 类型 | 说明 |
|------|------|
| `ErrorType` | 致命错误分类：`authentication`/`permission`/`rate_limit`/`invalid_request`/`upstream`/`connection`/`internal`/`unknown` |
| `ErrorInfo { error_type, message }` | 面向 UI 的结构化错误描述；`error_type` 序列化时字段名为 `"type"`，缺省为 `unknown` |
| `ReplyFinishedReason` | `completed`/`interrupted`/`exceed_max_iters`/`error` |

## 3. 快速示例 (Quick Example)

以下片段来自会话示例的真实代码——构造一个只含文本块的消息体（`Vec<ContentBlock>`）：

<!-- source: examples/session_test.rs:L44-L46 -->
```rust
fn make_msg(text: &str) -> Vec<ContentBlock> {
    vec![ContentBlock::Text(TextBlock::new(text.into()))]
}
```

完整上下文见 `examples/session_test.rs`（L157 起的 `run_save_load_roundtrip` 将此类消息写入会话并验证序列化往返）。

## 4. 关键用法模式 (Usage Patterns)

### 4.1 使用工厂函数创建消息

`agent_scope_message` 提供三个角色的工厂函数，自动处理校验与时间戳：

```rust
use agent_scope_message::{user_msg, system_msg, assistant_msg};

// user / system 工厂返回 Result（内容块可能违反角色校验）
let user = user_msg("user", "你好")?;
let sys = system_msg("system", "你是 helpful assistant")?;

// assistant 工厂不返回 Result（Assistant 接受全部块类型）
// 且 finished_at 保持 None——assistant 消息随流式生成逐步构建
let asst = assistant_msg("assistant", "你好！");
```

行为差异（源码 `crates/agent_scope_message/src/factory.rs`）：`user_msg`/`system_msg` 将 `finished_at` 设为 `created_at`（创建即定稿）；`assistant_msg` 的 `finished_at` 为 `None`（增量构建，定稿时再写入）。

### 4.2 按类型过滤与检查内容块

```rust
use agent_scope_message::BlockType;

// Some(block_type) 只返回该类型；None 返回全部
let texts = msg.get_content_blocks(Some(BlockType::Text));
let has_tool_call = msg.has_content_blocks(Some(BlockType::ToolCall));
let is_empty = !msg.has_content_blocks(None);
```

### 4.3 提取纯文本内容

`get_text_content(separator)` 拼接全部 `TextBlock` 的文本，无文本块时返回 `None`：

```rust
if let Some(text) = msg.get_text_content("\n") {
    println!("{text}");
}
```

### 4.4 处理未知块类型（前向兼容）

反序列化第三方或新版本协议产生的消息时，未知块类型不会导致解析失败：

```rust
// 含 "type": "image"（本版本未知）的 JSON 仍可反序列化为 Msg
// 未知块成为 ContentBlock::Unknown，遍历时以 block_type() == BlockType::Text 回退
for block in msg.get_content_blocks(None) {
    match block {
        ContentBlock::Text(t) => { /* ... */ }
        ContentBlock::ToolCall(tc) => { /* ... */ }
        _ => { /* Unknown 及其他变体 */ }
    }
}
```

注意：`Unknown` 变体不保留原始字段——需要无损处理未知协议时，请改用 `serde_json::Value` 直接解析。

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误类型 | 触发条件 |
|----------|----------|
| `ValidationError::InvalidContentForRole { role, disallowed_types }` | `Msg::new`（及 `user_msg`/`system_msg` 工厂）的内容块违反角色校验 |
| `ValidationError::EmptyContent` | 消息内容为空（可选检查，当前构造路径不强制） |
| `AppendEventError::ReplyIdMismatch` | 事件 `reply_id` 与消息 `id` 不匹配（事件流应用到消息时） |
| `AppendEventError::BlockNotFound` | 事件引用的内容块不存在 |
| `AppendEventError::UnknownEventType` | 无法识别的事件类型 |

**不支持的能力**：无。本模块为纯数据协议，无返回 `UnsupportedFeature` 的路径。

**常见问题**：

- *反序列化含未知块的消息后部分数据"消失"*：未知块被吸收为 `ContentBlock::Unknown`，不保留原始字段（见 4.4）。
- *`assistant_msg` 为什么不能返回错误*：Assistant 角色接受全部块类型，校验必然通过，工厂内部直接 `expect`。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L1**（数据结构逐字段兼容；消息工厂函数与角色细分为 **L2**——行为等价、签名 Rust 化）
- **权威来源**: `specs/001-compatibility-baseline/capability-matrix.json`
- **已知偏差**:
  - 矩阵 `status` 字段当前全部为 `NOT_ANALYZED`（矩阵未随 Feature 001-017 回填，回填列为后续任务）；本页等级以矩阵 `target_level`（`message-*`/`types-*` 条目为 LL1/LL2）+ `specs/002-foundation-layer` + 代码实际状态交叉核实为准。
  - `ContentBlock::Unknown` 前向兼容占位为 Rust 侧增强——Python 侧未知块类型的处理路径不同；Rust 吸收未知块但重新序列化时有损（不保留原始字段）。
  - `assistant_msg`/`assistant_msg_with_blocks` 不返回 `Result`（内部 `expect`）——与 `user_msg`/`system_msg` 签名不对称，属 Rust 惯用化差异。
  - `ToolCallBlock.input` 为未解析的原始 JSON 字符串（Foundation 层不解析工具参数）——参数解析发生在 Tool 层。
- **不支持的能力**: 无。

## 7. 相关模块 (See Also)

- [事件与流式 / event-streaming](./event-streaming.md) — 消息增量构建所依赖的事件体系
- [Agent 系统 / agent](./agent.md) — 消息的生产者与消费者
- [模型抽象 / model](./model.md) — `Usage` 与 `finished_reason` 的写入方
- [工具系统 / tool](./tool.md) — `ToolCallBlock`/`ToolResultBlock` 的生命周期
