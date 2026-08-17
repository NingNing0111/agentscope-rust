---
title: "工具内置检查"
description: "工具在运行时对自身输入执行的安全分析"
---

<Note>
**Rust 实现状态**: 部分支持。已支持：工具只读标记（`Tool::is_read_only()`）与权限引擎的基础判定。尚未实现：更细粒度的运行时「只读命令自动放行」「危险路径安全 ASK」「工作目录内编辑自动放行」内建检查。
</Note>

在规则与模式之外，工具自身还暴露只读标记，支撑权限系统的只读快速通道：

## 只读标记

`Tool` trait 提供 `is_read_only()`，声明该工具是否只读、不产生副作用：

```rust
use agent_scope_tool::Tool;

fn is_read_only(tool: &dyn Tool) -> bool {
    tool.is_read_only()
}
```

- 内置工具按性质声明只读（如 `Read`、`Grep`、`Glob`）。
- MCP 工具通过 `is_read_only()` 反映远程工具声明的只读性。
- 自定义工具可覆写 `is_read_only()` 与 `is_concurrency_safe()`。

## 边界

运行时对工具**真实入参**的动态分析（解析 Bash 命令判只读、敏感路径检测、工作目录自动放行）在 Rust 中为「部分支持」。Rust 版当前以静态只读标记 + 权限规则为主，需要时可在自定义工具内自行实现入参检查。

## 完整示例

见 [`examples/tool`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/tool/) 与 [`examples/mcp`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/mcp/)，其中打印每个工具的 `is_read_only()`。
