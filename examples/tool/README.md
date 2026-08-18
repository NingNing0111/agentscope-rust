# `tool` 示例

自定义工具示例：把普通 async Rust 函数封装成 `FunctionTool`，注册到 `ToolKit`，再交给 `ReActAgent` 在对话中自动调用。

## 运行

```bash
cargo run -p tool -- --prompt "请使用 calculator 工具计算 6 * 7，并只用一句话给出结果。"
```

## 凭据

真实模型调用需要环境变量：

```bash
export DEFAULT_API_KEY="sk-your-key"
# 可选：覆盖默认模型和 OpenAI-compatible endpoint
export DEFAULT_CHAT_MODEL="qwen3.7-plus"
export DEFAULT_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
```

缺失凭据时程序会给出明确错误提示（不会静默失败或 panic）。

## 预期行为

- 程序先打印交给 agent 前当前 `ToolKit` 的 OpenAI-compatible JSON Schema：包括示例注册的 `calculator` / `read_file`，以及 `ToolKit::new()` 自动包含的 `Skill`；这一步不包含 agent 构造后可能加入的任务工具。
- 有凭据时：`ReActAgent` 根据 prompt 调用 `calculator`，输出工具调用与工具结果事件，然后给出最终回复。
- 无凭据时：输出明确的 `DEFAULT_API_KEY` 缺失错误。
