# `mcp` 示例

MCP 客户端接入示例：通过 McpClient 连接 stdio MCP server，将远程工具接入 ToolKit 并调用。

## 运行

```bash
cargo run -p mcp -- --help
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
