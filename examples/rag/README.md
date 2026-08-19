# `rag` 示例

检索增强问答示例：嵌入模型 + TurbovecVectorStore + KnowledgeBase + RAGMiddleware（Static）。

## 运行

```bash
cargo run -p rag -- --help
```

从本地文件解析并入库（纯文本默认可用；PDF / Office / HTML 需要 xberg）：

```bash
cargo run -p rag --features xberg -- --file ./notes.pdf --prompt "文档讲了什么？"
```

## 凭据

真实模型调用需要环境变量：

```bash
export DEFAULT_API_KEY="sk-your-key"
```

可选：`DEFAULT_CHAT_MODEL`、`DEFAULT_EMBEDDING_MODEL`、`DEFAULT_URL`。

缺失凭据时程序会给出明确错误提示（不会静默失败或 panic）。

## 预期行为

- 有凭据且未指定 `--file`：写入两段内置知识，并基于知识库回答
- 有凭据且指定 `--file`：解析该文件后入库并回答
- 无凭据时：输出明确的缺凭据错误
