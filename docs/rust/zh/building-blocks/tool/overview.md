---
title: "概述"
description: "通过 ToolKit 为智能体装配工具、MCP 服务与技能"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。
</Note>

工具是智能体与外部世界交互的途径：执行 shell 命令、读写文件、调用 API。每个工具通过 JSON Schema 向大模型描述自己的用途与参数，智能体再通过统一的 `Tool` 接口发起调用，框架负责把模型传回的 JSON 参数解析成类型化输入并执行。

AgentScope Rust 在 `agent_scope_tool` crate 中提供三个与工具相关的核心概念：

| 概念 | 职责 |
|------|------|
| **工具（Tool）** | 实现 `Tool` trait 的对象：既包括 Bash / Read / Write 等内置工具，也包括把 Rust 异步函数包装成工具的 `FunctionTool`、把远程 MCP 工具适配进来的 `McpTool` |
| **ToolKit** | 工具注册表：负责注册工具、MCP 客户端与技能，向模型导出它们的 JSON Schema，并把每次工具调用分发给对应的工具对象 |
| **工具组（Tool Group）** | 一组工具的集合，可作为整体被激活或停用；智能体在运行时通过内置元工具 `ResetTools` 切换工具组 |

## Tool trait：统一契约

任何工具都是一个实现了 `Tool` trait 的对象。它由三部分组成：描述自己的元数据（名称、说明、参数 schema）、一些能力标记（是否只读、是否并发安全），以及真正执行调用的 `call` 方法。

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `name()` | `&str` | 暴露给智能体的工具名称，也是 `ToolKit` 中的唯一键 |
| `description()` | `&str` | 面向智能体的功能描述，帮助模型判断何时该调用它 |
| `input_schema()` | `JsonValue` | 定义输入参数的 JSON Schema（`{"type":"object","properties":{...},"required":[...]}`） |
| `is_concurrency_safe()` | `bool` | 能否安全地被多个异步任务并发调用，默认 `true` |
| `is_read_only()` | `bool` | 是否无副作用（不会修改外部状态），默认 `false` |
| `is_external_tool()` | `bool` | 是否在智能体进程之外执行，默认 `false` |
| `call(input)` | `Result<ToolExecOutput, ToolError>` | 执行一次工具调用，接收一个 `serde_json::Value` 参数对象 |

一次 `call` 的返回值是 `ToolExecOutput`，它有两种形态：`Complete`（一次性返回完整结果）或 `Stream`（流式输出，由调用方逐块消费）。所有失败都通过类型化的 `ToolError` 表达（例如工具不存在 `NotFound`、参数非法 `InvalidInput`、执行失败 `Execution` 等），而不是直接 panic。

## 快速上手

最简单的 `ToolKit` 只需注册一个工具：

```rust
use agent_scope_tool::{FunctionTool, ToolKit};

let mut toolkit = ToolKit::new();
toolkit.register(FunctionTool::new("calculator", "Evaluate a math expression.", calculator));
```

注册到 `"basic"` 组的工具始终激活。当绑定工作空间（workspace）时，内置工具（Bash / Read / Write / Edit / Grep / Glob / ResetTools / Skill）会自动注入（见 [工作空间](../workspace/overview)）。

## 延伸阅读

每种能力来源都有独立页面介绍：

<CardGroup :cols="2">
  <Card title="函数工具" icon="code" href="/building-blocks/tool/python-tool">
    内置工具、自定义工具与函数包装。
  </Card>
  <Card title="MCP" icon="plug" href="/building-blocks/tool/mcp">
    接入 MCP 服务并使用其工具。
  </Card>
  <Card title="Skill" icon="book-open" href="/building-blocks/tool/skill">
    用 Markdown 指令集拓展智能体能力。
  </Card>
  <Card title="元工具" icon="toggle-on" href="/building-blocks/tool/manage-tools">
    让智能体在运行时激活或停用工具组。
  </Card>
</CardGroup>
