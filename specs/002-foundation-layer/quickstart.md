# Quickstart: AgentScope Foundation Layer

**Feature**: 002-foundation-layer | **Date**: 2026-07-28

## Prerequisites

- Rust toolchain (stable 1.85+): `rustc --version`
- `cargo` 构建工具
- 已 clone AgentScope Python 参考实现（用于黄金快照生成）

## Getting Started

### 1. 构建 Foundation Layer

```bash
# 在项目根目录
cargo build

# Foundation 层作为 library crate，可通过以下方式验证编译：
cargo check
```

### 2. 运行单元测试

```bash
# 运行所有 Foundation 层测试
cargo test

# 按模块运行
cargo test -- types      # Types 模块
cargo test -- message    # Message 模块
cargo test -- event      # Event 模块
cargo test -- state      # State 模块

# 运行特定测试
cargo test -- test_msg_creation
cargo test -- test_content_block_serde
cargo test -- test_event_append
```

### 3. 运行序列化往返测试

```bash
# 验证每个数据结构的 JSON 序列化→反序列化→再序列化一致性
cargo test -- serialization
cargo test -- roundtrip
```

### 4. 运行差分测试（Rust vs Python 黄金快照）

```bash
# 生成 Python 黄金快照（在 Python 环境中）
cd agentscope/
python scripts/generate_golden_fixtures.py --types message event state \
    --output ../tests/compatibility/fixtures/

# 回到 Rust 项目运行差分测试
cargo test -- compatibility

# 测试比较 Rust 序列化输出与 Python 黄金快照（经归一化规则处理）
```

### 5. 代码质量检查

```bash
cargo clippy -- -D warnings
cargo fmt -- --check
```

## Validation Scenarios

### Scenario 1: 创建和操作消息

```rust
use agent_scope::message::*;
use agent_scope::types::ReplyFinishedReason;

// 创建 user 消息（工厂函数）
let msg = user_msg("user", "Hello, what is the weather?")
    .expect("user 消息应成功创建");

assert_eq!(msg.role, Role::User);
assert!(msg.has_content_blocks(Some(BlockType::Text)));
assert_eq!(msg.get_text_content(" ").unwrap(), "Hello, what is the weather?");
assert!(msg.finished_at.is_some());  // user 消息默认 finished_at == created_at

// 创建 assistant 消息
let msg = assistant_msg(
    "assistant",
    vec![
        TextBlock::new("The weather is sunny.")
    ],
);

assert_eq!(msg.role, Role::Assistant);
assert!(msg.finished_at.is_none());  // assistant 消息没有默认 finished_at

// 创建 system 消息
let msg = system_msg("system", "You are a helpful assistant.")
    .expect("system 消息应成功创建");
assert_eq!(msg.role, Role::System);
```

### Scenario 2: ContentBlock 过滤

```rust
// 创建包含多种 ContentBlock 的消息
let msg = assistant_msg(
    "alice",
    vec![
        TextBlock::new("I will search for you").into(),
        ToolCallBlock {
            id: "tc-001".into(),
            name: "search".into(),
            input: r#"{"q":"weather"}"#.into(),
            state: ToolCallState::Pending,
            suggested_rules: vec![],
            created_at: "2026-07-28T10:00:00Z".into(),
            finished_at: None,
        }.into(),
        TextBlock::new("Here are results:").into(),
    ],
);

// 过滤文本块
let texts = msg.get_content_blocks(Some(BlockType::Text));
assert_eq!(texts.len(), 2);

// 过滤工具调用块
let tool_calls = msg.get_content_blocks(Some(BlockType::ToolCall));
assert_eq!(tool_calls.len(), 1);
```

### Scenario 3: 流式事件驱动消息构建

```rust
use agent_scope::event::*;

let mut msg = Msg::new_assistant("agent");
msg.id = "reply-001".into();

// 模拟流式事件序列
msg.append_event(&AgentEvent::TextBlockStart(TextBlockStartEvent {
    base: event_base(),
    reply_id: "reply-001".into(),
    block_id: "block-001".into(),
})).unwrap();

msg.append_event(&AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
    base: event_base(),
    reply_id: "reply-001".into(),
    block_id: "block-001".into(),
    delta: "Hel".into(),
})).unwrap();

msg.append_event(&AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
    base: event_base(),
    reply_id: "reply-001".into(),
    block_id: "block-001".into(),
    delta: "lo".into(),
})).unwrap();

msg.append_event(&AgentEvent::TextBlockEnd(TextBlockEndEvent {
    base: event_base(),
    reply_id: "reply-001".into(),
    block_id: "block-001".into(),
})).unwrap();

// 验证最终消息
assert_eq!(msg.get_text_content(" ").unwrap(), "Hello");
assert!(msg.has_content_blocks(Some(BlockType::Text)));
```

### Scenario 4: JSON 序列化往返

```rust
// 序列化为 JSON
let json_str = serde_json::to_string(&msg).unwrap();

// 反序列化
let restored: Msg = serde_json::from_str(&json_str).unwrap();

// 验证
assert_eq!(restored.role, msg.role);
assert_eq!(restored.content.len(), msg.content.len());
assert_eq!(restored.get_text_content(" "), msg.get_text_content(" "));
```

### Scenario 5: AgentState 操作

```rust
use agent_scope::state::*;

let mut state = AgentState::new();

// 追加 context
state.append_context(
    "agent",
    vec![TextBlock::new("Task complete").into()]
).unwrap();

assert_eq!(state.context_length(), 1);

// 检查待处理工具调用
assert!(!state.has_awaiting_tool_calls("agent"));
```

### Scenario 6: ToolCallBlock 状态机

```rust
use agent_scope::message::ToolCallState;

// 创建 PENDING 状态的 ToolCallBlock
let mut tc = ToolCallBlock {
    id: "tc-1".into(),
    name: "search".into(),
    input: r#"{"q":"test"}"#.into(),
    state: ToolCallState::Pending,
    suggested_rules: vec![],
    created_at: "2026-07-28T10:00:00Z".into(),
    finished_at: None,
};

assert_eq!(tc.state, ToolCallState::Pending);

// 验证 ToolCallState 枚举值序列化
let json = serde_json::to_string(&ToolCallState::Pending).unwrap();
assert_eq!(json, r#""pending""#);

let json = serde_json::to_string(&ToolCallState::Finished).unwrap();
assert_eq!(json, r#""finished""#);
```

### Scenario 7: 验证规则

```rust
// user 消息不允许 tool_call 块
let result = user_msg("user", vec![
    ToolCallBlock { /* ... */ },
]);
assert!(result.is_err());

// system 消息不允许 data 块
let result = system_msg("system", vec![
    DataBlock { /* ... */ },
]);
assert!(result.is_err());

// assistant 消息允许所有块类型
let msg = assistant_msg("assistant", vec![
    TextBlock::new("text"),
    ToolCallBlock { /* ... */ },
]);
assert!(msg.role == Role::Assistant);  // 不应报错
```

## Expected Outcomes

运行 `cargo test` 后，应看到：

```text
test result: ok. ... passed; ... failed; ... ignored; 0 measured; 0 filtered out
```

所有测试通过表示 Foundation 层的数据结构满足：
- ✅ 正确的类型定义和字段结构
- ✅ JSON 序列化/反序列化一致性
- ✅ ContentBlock 的多态序列化（tagged enum）
- ✅ 事件类型的完整枚举
- ✅ Msg 的 role-based 验证
- ✅ append_event 的增量消息构建
- ✅ AgentState 的状态管理方法
- ✅ ToolCallBlock 的状态机
