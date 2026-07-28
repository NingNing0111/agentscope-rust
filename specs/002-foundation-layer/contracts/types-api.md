# Types Module API Contract

**Module**: `agent_scope::types` | **Dependencies**: None (no agentscope internal deps)

## Public Types

### Enums

```rust
/// 回复终止原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyFinishedReason {
    Completed,
    Interrupted,
    ExceedMaxIters,
    Error,
}
```

```rust
/// 错误分类（HTTP 语义对齐）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    Authentication,  // 401
    Permission,      // 403
    RateLimit,       // 429
    InvalidRequest,  // 400/422
    Upstream,        // 5xx
    Connection,      // 网络错误
    Internal,        // 框架内部错误
    Unknown,         // 兜底
}
```

### Structs

```rust
/// 结构化错误信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    #[serde(default)]
    pub error_type: ErrorType,  // default: Unknown
    pub message: String,
}
```

### Type Aliases

```rust
/// Embedding 向量
pub type Embedding = Vec<f64>;

/// 用户生成内容的类型限制（不暴露 serde_json::Value 到公共 API）
/// 内部可用于约束内容类型
pub type JsonValue = serde_json::Value;
```

### Hook Types

```rust
/// Agent Hook 点类型（6 种）
pub type AgentHookType = &'static str;

/// 预定义的 Agent Hook 点
pub mod agent_hooks {
    pub const PRE_REPLY: &str = "pre_reply";
    pub const POST_REPLY: &str = "post_reply";
    pub const PRE_PRINT: &str = "pre_print";
    pub const POST_PRINT: &str = "post_print";
    pub const PRE_OBSERVE: &str = "pre_observe";
    pub const POST_OBSERVE: &str = "post_observe";
}

/// ReAct Agent Hook 点类型（在 AgentHook 基础上增加 4 种）
pub mod react_agent_hooks {
    pub use super::agent_hooks::*;
    pub const PRE_REASONING: &str = "pre_reasoning";
    pub const POST_REASONING: &str = "post_reasoning";
    pub const PRE_ACTING: &str = "pre_acting";
    pub const POST_ACTING: &str = "post_acting";
}
```

注：Python 实现中使用 `Literal[...]` 联合类型。Rust 等效使用 `&'static str` + const 常量，在编译时通过 `AgentHookType` 类型提供 API 层面的文档约束，但不强制执行（Rust 无 equivalent to Literal union in stable）。

## JSON Serialization Contracts

### ReplyFinishedReason 示例

```json
"completed"           // ReplyFinishedReason::Completed
"interrupted"         // ReplyFinishedReason::Interrupted
"exceed_max_iters"    // ReplyFinishedReason::ExceedMaxIters
"error"               // ReplyFinishedReason::Error
```

### ErrorType 示例

```json
"authentication"      // ErrorType::Authentication
"rate_limit"          // ErrorType::RateLimit
"invalid_request"     // ErrorType::InvalidRequest
"unknown"             // ErrorType::Unknown
```

### ErrorInfo 示例

```json
{
  "type": "rate_limit",
  "message": "Too many requests"
}
```

```json
{
  "type": "unknown",
  "message": "An unexpected error occurred"
}
```

### Embedding 示例

```json
[0.12, -0.45, 0.78, 0.01]
```

## Dependency Boundary

Types 模块依赖：
- ✅ `serde`, `serde_json`（框架级依赖，宪法允许）
- ✅ `uuid`, `chrono`（ID 和时间戳工具库）
- ❌ No `agent_scope::message`
- ❌ No `agent_scope::event`
- ❌ No `agent_scope::state`
- ❌ No `agent_scope::model` or any other agentscope internal module
