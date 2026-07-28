# Quickstart: Tool System 验证指南

**Feature**: 006-tool-system | **Date**: 2026-07-29

本文档描述了如何使用 `agent_scope_tool` crate 的基本功能，并提供了可运行的验证场景。

## 前提条件

- Rust toolchain (stable)
- 项目已 `cargo build` 通过

## 场景 1: 创建并调用 FunctionTool（US1）

### 1.1 定义输入结构体

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SearchInput {
    query: String,
    max_results: Option<usize>,
}
```

### 1.2 用 FunctionTool::new() 包装 handler

```rust
use agent_scope_tool::{FunctionTool, Tool};

async fn search_handler(input: SearchInput) -> String {
    format!("Results for '{}': found {} items", input.query, input.max_results.unwrap_or(5))
}

let tool = FunctionTool::new(
    "web_search",
    "Search the web for information",
    search_handler,
);
```

### 1.3 验证元数据

```rust
assert_eq!(tool.name(), "web_search");
assert_eq!(tool.description(), "Search the web for information");

let schema = tool.input_schema();
assert_eq!(schema["type"], "object");
assert!(schema["properties"]["query"]["type"] == "string");
```

### 1.4 调用 Tool

```rust
use serde_json::json;

let result = tool.call(json!({"query": "rust", "max_results": 3})).unwrap();

match result {
    ToolExecOutput::Complete(chunk) => {
        assert_eq!(chunk.state, ToolResultState::Success);
        assert!(chunk.is_last);
        match &chunk.output {
            ToolOutput::Text(text) => {
                println!("Tool output: {}", text);
                assert!(text.contains("Results for 'rust'"));
            }
            _ => panic!("Expected text output"),
        }
    }
    _ => panic!("Expected Complete variant"),
}
```

### 运行验证

```bash
cargo test -p agent_scope_tool -- --test test_function_tool_new
```

---

## 场景 2: ToolKit 注册与 Schema 导出（US2）

### 2.1 注册多个 Tool

```rust
use agent_scope_tool::{ToolKit, FunctionTool};

let mut toolkit = ToolKit::new();
assert_eq!(toolkit.len(), 0);
assert!(toolkit.is_empty());

let search = FunctionTool::new("search", "Search tool", search_handler);
let calc = FunctionTool::new("calc", "Calculator tool", calc_handler);

toolkit.register(search);
toolkit.register(calc);

assert_eq!(toolkit.len(), 2);
assert!(toolkit.contains("search"));
assert!(toolkit.contains("calc"));
assert!(!toolkit.contains("unknown"));
```

### 2.2 导出 OpenAI Schema

```rust
let schemas = toolkit.get_tool_schemas();
assert_eq!(schemas.len(), 2);

let first = &schemas[0];
assert_eq!(first["type"], "function");
assert_eq!(first["function"]["name"], "search");
assert!(first["function"]["parameters"].is_object());
```

### 2.3 通过 ToolCallBlock 调用

```rust
use agent_scope_message::ToolCallBlock;

let tc = ToolCallBlock::new(
    "tc-1".into(),
    "search".into(),
    r#"{"query":"test"}"#.into(),
);

let result = toolkit.call_tool(&tc).unwrap();
// ... verify result
```

### 2.4 错误处理

```rust
// Missing tool
let bad_call = ToolCallBlock::new("tc-2".into(), "nonexistent".into(), "{}".into());
let err = toolkit.call_tool(&bad_call).unwrap_err();
assert!(matches!(err, ToolError::NotFound { .. }));

// Invalid input
let bad_input = ToolCallBlock::new("tc-3".into(), "search".into(), "not valid json".into());
```

### 2.5 名称覆盖

```rust
let v2 = FunctionTool::new("search", "Search v2", search_handler_v2);
toolkit.register(v2);  // overwrites old "search"
assert_eq!(toolkit.len(), 2);
```

### 运行验证

```bash
cargo test -p agent_scope_tool -- --test test_toolkit
```

---

## 场景 3: ChatModel 集成验证（US3）

### 3.1 Schema 与 ChatModel 格式兼容性

```rust
// toolkit.get_tool_schemas() output is directly usable as ChatModel::call(tools) parameter
let schemas = toolkit.get_tool_schemas();
let schemas_ref: Vec<&serde_json::Value> = schemas.iter().collect();

// Verify that DashScopeChatModel::build_request_body() accepts this format
// (existing tests in agent_scope_dashscope cover tool serialization)
```

### 3.2 ToolCallBlock → ToolKit 闭环

```rust
// Simulate: model returns ToolCallBlock → ToolKit::call_tool executes it
let tc = ToolCallBlock::new("tc-1".into(), "calc".into(), r#"{"op":"add","a":1,"b":2}"#.into());
let result = toolkit.call_tool(&tc).unwrap();
// Verify handler returned expected output
```

### 3.3 ToolChoice 验证

```rust
// Verify ToolChoice::validate() works with toolkit's schema output
let tool_names: Vec<String> = toolkit.get_tool_schemas().iter()
    .filter_map(|s| s["function"]["name"].as_str())
    .map(|s| s.to_string())
    .collect();

let tc = ToolChoice::specific_tool("search");
assert!(tc.validate(Some(&tool_names)).is_ok());

let tc_bad = ToolChoice::specific_tool("missing");
assert!(tc_bad.validate(Some(&tool_names)).is_err());
```

### 运行验证

```bash
cargo test -p agent_scope_tool -- --test test_integration_with_model
```

---

## 场景 4: Edge Cases

### 4.1 Handler panic → ToolError::Execution

```rust
fn panicking_handler(_input: SearchInput) -> String {
    panic!("intentional panic for testing");
}

let tool = FunctionTool::new("panic_test", "Will panic", panicking_handler);
let result = tool.call(json!({"query": "test"}));
assert!(matches!(result, Err(ToolError::Execution { .. })));
```

### 4.2 空 ToolKit

```rust
let toolkit = ToolKit::new();
assert_eq!(toolkit.get_tool_schemas().len(), 0);
assert!(toolkit.is_empty());
```

### 4.3 clear() 后 Toolkit 为空

```rust
toolkit.clear();
assert_eq!(toolkit.len(), 0);
```

### 4.4 无效 JSON input → ToolError::InvalidInput

```rust
let tc = ToolCallBlock::new("tc-x".into(), "search".into(), r#"not json"#.into());
let err = toolkit.call_tool(&tc).unwrap_err();
assert!(matches!(err, ToolError::InvalidInput { .. }));
```

### 运行验证

```bash
cargo test -p agent_scope_tool -- --test test_edge_cases
```

---

## 完整运行全部测试

```bash
# 仅 tool crate
cargo test -p agent_scope_tool

# workspace 全量
cargo test --workspace

# 带 lint
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

---

## 预期结果

| 验证项 | 预期 |
|--------|------|
| Tool trait 创建 | 通过 `FunctionTool::new()` 创建，元数据正确 |
| input_schema() 格式 | JSON Schema `{"type": "object", "properties": {...}, "required": [...]}` |
| ToolKit::get_tool_schemas() | OpenAI function schema 格式数组 |
| ToolCallBlock → ToolKit::call_tool() | 正确分发并返回结果 |
| Missing tool | `Err(ToolError::NotFound)` |
| Invalid input | `Err(ToolError::InvalidInput)` |
| Handler panic | `Err(ToolError::Execution)` |
| 名称覆盖 | 新 Tool 覆盖旧 Tool |
| ChatModel 集成 | Schema 可直接用于 `ChatModel::call(tools)` |
| clippy + fmt | 零警告 |

## 成功标准 (from spec)

- SC-001: `agent_scope_tool` crate 编译通过，无 warning
- SC-002: `FunctionTool::new()` 创建 Tool，`input_schema()` 正确生成
- SC-003: `ToolKit::get_tool_schemas()` 输出与 Python 格式一致
- SC-004: `ToolKit::call_tool()` 可执行已注册 Tool
- SC-005: 所有 test 在无网络环境下可运行
- SC-006: `cargo clippy --workspace` 和 `cargo fmt --all -- --check` 全通过
