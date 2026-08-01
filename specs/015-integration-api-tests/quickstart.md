# Quickstart: Integration API Tests (Examples)

**Feature**: 015-integration-api-tests
**Date**: 2026-07-31

## Prerequisites

1. Rust stable toolchain（`rustc --version` ≥ 1.85）
2. DashScope API key（从 [阿里云百炼](https://bailian.console.aliyun.com/) 获取，格式 `sk-xxx`）
3. 网络能访问 `dashscope.aliyuncs.com`

## Run All Examples

```bash
# 设置 API key（二选一）
export API_KEY="sk-xxxxxxxxxxxxxxxxxxxxxxxx"

# 或者每次指定 --api-key
```

### Example 1: Memory Integration

验证记忆系统（创建、检索、跨对话）与真实 LLM 集成。

```bash
# 通过环境变量
cargo run --example memory_test

# 通过命令行参数
cargo run --example memory_test -- --api-key sk-xxx --model qwen-plus
```

**预期输出**:
```
── 1. Write Memory ──
  ✓ Write Memory (X.Xs)
    Agent confirmed: ...
── 2. Search Memory ──
  ✓ Search Memory (X.Xs)
    Found reference to ...
── 3. Memory Reasoning ──
  ✓ Memory Reasoning (X.Xs)
    Agent used stored memory

ALL 3 TESTS PASSED (Y.Ys total)
```

### Example 2: Session Persistence

验证会话保存/加载与对话历史保持。

```bash
cargo run --example session_test
```

**预期输出**:
```
── 1. Session Save/Load Roundtrip ──
  ✓ Save/Load (X.Xs)
    Session context preserved: 2 messages
── 2. Loaded Context Consistency ──
  ✓ Context Consistency (X.Xs)
    Agent remembered: ...
── 3. Session Close & Cleanup ──
  ✓ Close & Cleanup (X.Xs)
    Session closed, store empty

ALL 3 TESTS PASSED (Y.Ys total)
```

### Example 3: RAG Pipeline

验证文档索引、检索、基于知识的回答。

```bash
cargo run --example rag_test
```

**预期输出**:
```
── 1. Ingest Document ──
  ✓ Ingest (X.Xs)
    Indexed 3 chunks
── 2. Grounded Query ──
  ✓ Grounded Query (X.Xs)
    Answer contains: ...
── 3. Empty KB Query ──
  ✓ Empty KB Query (X.Xs)
    Agent responded normally without errors

ALL 3 TESTS PASSED (Y.Ys total)
```

### Example 4: Streaming Tool-Call Events

验证流式工具调用的完整事件生命周期。

```bash
cargo run --example streaming_tool_test
```

**预期输出**:
```
── 1. Single Tool Call ──
  ✓ Single Tool Call (X.Xs)
    ToolCallStart=1, ToolCallEnd=1, ToolResultStart=1, ToolResultEnd=1
    Answer: 8.53452 ✓
── 2. Multi-Tool Call ──
  ✓ Multi-Tool Call (X.Xs)
    ToolCall cycles: 2, Answer correct

ALL 2 TESTS PASSED (Y.Ys total)
```

## 故障排除

| 现象 | 可能原因 | 解决方案 |
|------|---------|---------|
| `API key invalid` | API key 格式错误或已过期 | 检查 API key，确保以 `sk-` 开头 |
| `Connection timeout` | 网络不可达 | 检查网络，或设置代理 |
| `Model not found` | 模型名称错误 | 使用 `--model qwen-plus` |
| `Quota exceeded` | API 调用次数超限 | 检查 DashScope 控制台配额 |
| `Linker errors` | 缺少系统依赖 | `brew install openssl` (macOS) |
