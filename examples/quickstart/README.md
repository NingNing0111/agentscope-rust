# `quickstart` 示例

最小 Agent 示例：DashScopeChatModel + ToolKit + ReActAgent，演示 reply 与 reply_stream 两种用法。

## 运行

```bash
cargo run -p quickstart -- --help
```

## 凭据

真实模型调用需要环境变量：

```bash
export DASHSCOPE_API_KEY="sk-your-key"
```

缺失凭据时程序会给出明确错误提示（不会静默失败或 panic）。

## 预期行为

<!-- 实现时补充：运行命令 + 预期输出 -->
- 有凭据时：……
- 无凭据时：输出明确的缺凭据错误
