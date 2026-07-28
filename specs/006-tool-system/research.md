# Research: Tool System

**Feature**: 006-tool-system | **Date**: 2026-07-29

## Research Questions

### RQ-1: schemars 集成模式 — `T: JsonSchema` 自动提取 JSON Schema

**Decision**: 使用 `schemars::JsonSchema` derive macro + `schemars::schema_for!()` 宏。

**Rationale**: 
- `schemars` 是 Rust 生态中 JSON Schema 推导的标准方案
- 上游 AgentScope Python 使用 Pydantic 自动推导 schema，`schemars` 是其 Rust 对应物
- workspace `Cargo.toml` 中已有 `schemars = "0.8"` 依赖
- `FunctionTool::new::<T: JsonSchema>(...)` 通过 `schema_for!(T)` 获取 `RootSchema`，再 `.to_value()` 转为 `serde_json::Value`

**Alternatives considered**:
- 手写 schema 字符串 → 不可维护，不符合 spec 的"自动提取"要求
- `jsonschema` crate → 不如 schemars 成熟且 Rust 生态认可

**Implementation pattern**:
```rust
pub fn new<F, Fut, T, R>(
    name: impl Into<String>,
    description: impl Into<String>,
    handler: F,
) -> Self
where
    T: schemars::JsonSchema + for<'de> Deserialize<'de>,
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send,
    R: IntoChunk,
{
    let schema = schemars::schema_for!(T);
    let schema_value = serde_json::to_value(&schema)
        .expect("schemars RootSchema should serialize");
    // ...
}
```

---

### RQ-2: 返回值类型转换 — IntoChunk trait

**Decision**: 定义 `IntoChunk` trait，为 `String` 和 `ToolResultBlock` 分别实现。

**Rationale**:
- Spec FR-008/FR-009 要求 handler 返回 `String` 时自动转 `ToolResultBlock`，返回 `ToolResultBlock` 时直接透传
- trait 比宏或函数重载更类型安全、更符合 Rust 习惯
- 可扩展：未来可支持更多返回类型（如 `Vec<ToolResultBlock>`）

**Alternatives considered**:
- 泛型 + trait bounds 直接判断 → 编译器无法区分 String 和 ToolResultBlock 的重载
- 用 enum 包裹返回值 → 增加调用方负担
- `Into<ToolResultBlock>` trait → 可以用标准库的，但这样需要为 String 实现 `From<String> for ToolResultBlock`，语义上不够精确

**Implementation**:
```rust
pub trait IntoChunk: Send + 'static {
    fn into_chunk(self) -> ToolResultBlock;
}

impl IntoChunk for String {
    fn into_chunk(self) -> ToolResultBlock {
        ToolResultBlock {
            output: ToolOutput::Text(self),
            state: ToolResultState::Success,
            is_last: true,
            ..Default::default()
        }
    }
}

impl IntoChunk for ToolResultBlock {
    fn into_chunk(self) -> ToolResultBlock {
        self
    }
}
```

---

### RQ-3: Stream 模式 — ToolOutput enum 设计

**Decision**: `ToolOutput` enum 保持 `Complete(ToolResultBlock)` + `Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>)`。

**Rationale**:
- 与 `ModelCallResult::Complete(ChatResponse)` / `Stream(Pin<Box<dyn Stream<...>>>)` 模式一致（宪法 Art.8：保持 API 风格统一）
- `Pin<Box<dyn Stream>>` 是 Rust 异步生态的标准流式抽象
- Spec 明确要求此设计（FR-004）

**Alternatives considered**:
- `tokio::sync::mpsc::Receiver<ToolResultBlock>` → 限定了运行时实现
- `async-stream` 宏 + Generator → 过于侵入式

**Edge case**:
- Stream 消耗完但未标记 `is_last: true` → 由调用方（Agent）的判断逻辑，ToolKit 不做处理
- Stream → ToolKit 不自动累积 → 由调用方负责消费（spec 明确说明）

---

### RQ-4: ToolCallBlock input 反序列化

**Decision**: `ToolKit::call_tool()` 中直接 `serde_json::from_str::<T>(&tool_call.input)`。

**Rationale**:
- `ToolCallBlock.input` 是 `String`（原始 JSON 字符串）
- `FunctionTool::call()` 接收 `serde_json::Value`，内部通过 `serde_json::from_value::<T>(input)` 反序列化
- 反序列化失败 → `ToolError::InvalidInput`

**Alternatives considered**:
- Tool trait 的 `call()` 直接接收泛型参数 → 需要在 trait 层面引入泛型，牺牲 trait object 兼容性
- 在 `ToolKit` 层反序列化 → 需要知道每个 Tool 的目标类型，增加复杂度

**Implementation in Tool trait**:
```rust
fn call(&self, input: serde_json::Value) -> Result<ToolOutput, ToolError>;
```

The `FunctionTool` internally deserializes `JsonValue` → `T`:
```rust
let typed: T = serde_json::from_value(json_input)
    .map_err(|e| ToolError::InvalidInput {
        tool_name: self.name.clone(),
        reason: e.to_string(),
    })?;
```

---

### RQ-5: OpenAI Tool Schema 格式

**Decision**: `ToolKit::get_tool_schemas()` 输出格式：
```json
[{
  "type": "function",
  "function": {
    "name": "...",
    "description": "...",
    "parameters": { "type": "object", "properties": {...}, "required": [...] }
  }
}]
```

**Rationale**:
- 与 AgentScope Python `Toolkit.get_tool_schemas()` 输出一致（Art.1 兼容性优先）
- 与 `ChatModel::call_api(tools: Option<&[JsonValue]>)` 参数格式匹配（US3 集成验证）
- 现有 `agent_scope_model` 的 `build_request_body()` 已按此格式处理 tool schema

**Validate**: 检查 DashScope provider 的 `build_request_body` 如何处理 tools 参数，确认格式完全兼容（现有测试已覆盖）。

---

### RQ-6: Handler Panic 捕获

**Decision**: 使用 `std::panic::catch_unwind(AssertUnwindSafe(|| handler(...)))`。

**Rationale**:
- 宪法 Art.9 要求不使用 `unwrap()` — 但 handler 是外部代码，我们无法控制
- `catch_unwind` 是唯一合理的 panic 边界
- `AssertUnwindSafe` 在这里是安全的，因为我们在 panic 后不重复使用状态
- 转为 `ToolError::Execution`，不传播 panic 到调用方

**Alternatives considered**:
- `futures::FutureExt::catch_unwind()` → 对 async fn 更友好，但增加了依赖复杂度
- 不做保护 → 违反 FR-010

**Implementation**:
```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    futures::executor::block_on(handler(typed))
}));
match result {
    Ok(chunk) => Ok(ToolOutput::Complete(chunk.into_chunk())),
    Err(_panic) => Err(ToolError::Execution {
        tool_name: self.name.clone(),
        reason: "handler panicked".to_string(),
    }),
}
```

**Edge case**: catch_unwind 在 `panic = "abort"` 配置下无效 — 但这是 Rust 配置层面的选择，非 Tool 层应处理的。

---

### RQ-7: 命名冲突 — ToolOutput 名字冲突

**Decision**: `agent_scope_message` 中已有的 `ToolOutput` enum（表示 tool result 的输出内容：`Text(String)` / `Blocks(Vec<ToolResultBlockItem>)`）不与 tool crate 中的枚举冲突。Tool crate 中使用 `ToolCallOutput` 或直接返回 `Result<ToolResultBlock, ToolError>`。

**Wait — 再确认**: Spec FR-004 要求 `ToolOutput` enum 包含 `Complete(ToolChunk)` 和 `Stream(...)`。但 `agent_scope_message::ToolOutput` 是不同类型。这是两个不同 crate 中的独立类型，可以共存，但会造成混淆。

**Decision**: Tool trait 使用 `ToolExecResult` 作为执行结果类型（避免与 `agent_scope_message::ToolOutput` 混淆），在 tool crate 文档中说明差异。

**Rationale**:
- `agent_scope_message::ToolOutput` 是 ToolResultBlock 内部的 output 字段的类型（Text/Blocks）
- tool crate 中的枚举表达调用结果（Complete/Stream）
- 用不同名称区分，避免 `use` 冲突

**Updated spec mapping**:
- FR-004 中的 `ToolOutput` → 实现时命名为 `ToolExecResult` 或复用更精确的命名

**Actually**, let me reconsider. Looking at the spec more carefully:

FR-004: `ToolOutput` enum MUST contain `Complete(ToolChunk)` and `Stream(...)`

But then the Tool trait's `call()` return type must be `Result<ToolOutput, ToolError>` where `ToolOutput` is the call result enum. And `ToolResultBlock` is already in `agent_scope_message`.

Let me call it `ToolExecOutput` for the enum and keep `ToolChunk` as the alias for `ToolResultBlock`.

Wait, the spec explicitly says `ToolChunk` = `ToolResultBlock` type alias. So:

```rust
// In agent_scope_tool:
pub type ToolChunk = agent_scope_message::ToolResultBlock;

pub enum ToolExecOutput {
    Complete(ToolChunk),
    Stream(Pin<Box<dyn Stream<Item = Result<ToolChunk, ToolError>> + Send>>),
}
```

This avoids naming conflicts. The spec can be updated to use this name, or I can keep `ToolOutput` in the tool crate (since they're in different namespaces, rust manages this fine with `use agent_scope_tool::ToolOutput` vs `use agent_scope_message::ToolOutput`).

Let me go with keeping `ToolOutput` as the name in the tool crate. In practice, users won't import both in the same module often, and the full path disambiguates.

---

## Summary

| RQ | Decision | 
|-----|----------|
| RQ-1 | `schemars::schema_for!(T)` 自动生成 schema |
| RQ-2 | `IntoChunk` trait for String/ToolResultBlock |
| RQ-3 | `Pin<Box<dyn Stream>>` 对齐 ModelCallResult |
| RQ-4 | `call()` 接收 `serde_json::Value`，内部反序列化为 T |
| RQ-5 | OpenAI function schema 格式，与 Python 对齐 |
| RQ-6 | `std::panic::catch_unwind` + `AssertUnwindSafe` |
| RQ-7 | `ToolOutput`（tool crate）与 `agent_scope_message::ToolOutput` 不同 crate 可共存 |
