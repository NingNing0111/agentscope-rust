# Examples 目录 — 终端对话 Agent 设计

**日期**: 2026-07-31
**状态**: 已确认，待实现

---

## 概述

在 workspace 根目录创建 `examples/`，提供一个基于 DashScope API 的终端交互式对话 Agent，支持工具调用。

---

## 文件结构

```
examples/
├── lib.rs          # 共享构建逻辑（模型创建、Agent 构建、工具注册）
└── chat.rs         # 终端对话 binary
```

`examples/` 下的 `.rs` 文件由 Cargo 自动发现为 example binary，无需额外配置 `[[example]]`。

---

## API & 数据流

```
CLI args (--api-key, --model)
  → lib::create_model(api_key, model_name) → Arc<DashScopeChatModel>
  → lib::create_calculator_tool() → FunctionTool
  → ToolKit::new() + tk.register(tool)
  → lib::build_agent(model, toolkit) → ReActAgent
  
终端交互循环:
  stdin "> " → user_msg → agent.reply() → 打印 Msg 文本 → 循环
  exit/quit/Ctrl+C → 退出
```

---

## 组件

### `lib.rs` — 共享构建

三个公开函数：

1. **`create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel>`**
   - 创建 DashScopeChatModel，stream=true（模型侧启用流式）
   - temperature=0.7

2. **`create_calculator_tool() -> FunctionTool`**
   - 名称为 `calculator`，描述为 "Evaluate a mathematical expression"
   - 输入 schema: `{ expression: String }`（由 schemars 从 CalcInput 派生）
   - 实现：简易 f64 表达式求值（+、-、*、/、括号、幂运算），不使用外部 parser

3. **`build_agent(model: Arc<DashScopeChatModel>, toolkit: Option<ToolKit>) -> ReActAgent`**
   - AgentConfig: name="assistant", system_prompt 引导数学工具使用
   - ReActConfig + ContextConfig 使用默认值

### `chat.rs` — 终端对话

- **CLI**: clap derive 模式
  - `--api-key` / `-k`: 必填，DashScope API Key
  - `--model` / `-m`: 可选，默认 `qwen-plus`
- **交互循环**:
  - 提示符 `> `，stdin 逐行读取
  - 输入 `exit` / `quit` → 正常退出
  - 空白行 → 跳过
  - 其他 → `user_msg("user", input)` → `agent.reply(Some(vec![msg])).await`
  - 打印回复文本（`reply.get_text_content("")`）
  - 如果 Agent 内部有工具调用，ReActAgent 自动处理
- **运行时**: `#[tokio::main]` 单线程 runtime

---

## 工具：Calculator

```rust
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    /// Mathematical expression, e.g. "1 + 2 * 3"
    expression: String,
}
```

- 解析：`+`, `-`, `*`, `/`, `^`（幂），括号 `()`
- 运算类型：全部 `f64`
- 错误：返回 `"Error: <reason>"` 字符串
- 精度：保留 6 位小数

---

## 错误处理

| 场景 | 行为 |
|------|------|
| API Key 无效 | 打印友好消息并 exit(1) |
| 网络错误 | 打印 "请求失败: <reason>"，继续循环 |
| Agent 调用错误 | 打印 "Agent 错误: <reason>"，继续循环 |
| Ctrl+C | 打印换行，正常退出 |
| Calculator 解析失败 | 返回 "Error: <reason>" 给 Agent |

---

## 依赖变更

### workspace 根 `Cargo.toml` 新增：

```toml
[workspace.dependencies]
clap = { version = "4", features = ["derive"] }
```

### example 隐式依赖（workspace 成员自动可用）：

- `agent_scope_agent` — ReActAgent, AgentConfig
- `agent_scope_dashscope` — DashScopeChatModel
- `agent_scope_tool` — FunctionTool, ToolKit
- `agent_scope_message` — user_msg
- `tokio` — runtime
- `schemars` — JsonSchema derive
- `serde` / `serde_json` — 序列化

---

## 测试验证

- `rtk cargo build` — examples 编译通过
- `rtk cargo clippy` — 0 warnings
- 手动运行 `cargo run --example chat -- --api-key $DASHSCOPE_API_KEY` — 终端对话正常
