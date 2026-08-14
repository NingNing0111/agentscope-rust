---
title: "函数工具"
description: "把 Rust 函数包装为工具，自动生成 JSON Schema"
---

<Note>
**Rust 实现状态**: 部分支持（兼容等级 L3）。已支持：`FunctionTool`（把 Rust 异步函数包装为工具）、内置工具、工具中间件相关能力。尚未实现：Python 执行器与 Docker/E2B 等工具后端切换。
</Note>

工具是任意实现 `Tool` trait 的对象。AgentScope Rust 提供一组内置工具覆盖常见操作，并对外暴露统一接口供开发者自定义：

| 主题 | 内容 |
|------|------|
| Tool trait | 每个工具需要实现的方法 |
| 内置工具 | Bash / Read / Write / Edit / Grep / Glob / ResetTools / Skill（随 workspace 绑定注入） |
| 自定义工具 | 用 `FunctionTool` 把 Rust 异步函数变成工具 |

## Tool trait

`Tool` 是所有工具的抽象接口（见 [概述](overview)）。自定义工具需实现 `name()`、`description()`、`input_schema()`、`call()` 等。

## 将函数包装为工具

用 `FunctionTool` 把普通 Rust 异步函数变成工具——参数结构体只需实现 `Deserialize` + `JsonSchema`，schema 自动生成：

```rust
use agent_scope_tool::{FunctionTool, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    expression: String,
}

async fn calculator(input: CalcInput) -> String {
    format!("calced: {}", input.expression)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tool = FunctionTool::new("calculator", "Evaluate a math expression.", calculator);
    let out = tool.call(json!({ "expression": "6 * 7" })).await?;
    println!("{out:?}");
    Ok(())
}
```

## 内置工具

内置文件与命令工具通过绑定工作空间（workspace）自动注入，包括 `Bash`、`Read`、`Write`、`Edit`、`Grep`、`Glob`、`PowerShell`（仅 Windows 注入）、`ResetTools`、`Skill`。注意：`ListDir` 未作为内置工具提供，可用 `FunctionTool` 自定义或通过 workspace 文件操作实现。

## 完整示例

见 [`examples/tool`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/tool/)（`cargo run -p tool`），演示 `FunctionTool` 注册、ToolKit schema 输出与直接调用，无需模型凭据。
