# AgentScope Rust 文档 / AgentScope Rust Documentation

使用 Rust 重构的 AgentScope —— 在锁定的上游版本范围内，与 Python 参考实现保持外部可观察兼容的多智能体框架。

A Rust reimplementation of AgentScope — a multi-agent framework that stays observably compatible with the Python reference implementation within a pinned upstream version.

---

## 兼容基线 / Compatibility Baseline

| 项目 Item | 值 Value |
|-----------|----------|
| 上游仓库 Upstream | [agentscope-ai/agentscope](https://github.com/agentscope-ai/agentscope) |
| Release | v2.0.5 |
| Commit | `27b6a0d2a2afedf53462c9a2add33932d54b2d20` |
| Python | >=3.11 |
| 锁定日期 Locked | 2026-07-28 |

兼容性权威数据 / Authoritative compatibility data: `specs/001-compatibility-baseline/capability-matrix.json`

---

## 中文文档

**入口**：[快速上手](zh/getting-started.md) — 30 分钟跑通第一个 Agent

**推荐阅读顺序**：

1. [快速上手](zh/getting-started.md)
2. 模块文档（[modules/](zh/modules/)）：
   - [消息与基础类型](zh/modules/message-types.md)
   - [事件与流式](zh/modules/event-streaming.md)
   - [模型抽象](zh/modules/model.md)
   - [DashScope Provider](zh/modules/dashscope.md)
   - [工具系统](zh/modules/tool.md)
   - [Agent 系统](zh/modules/agent.md)
   - [记忆](zh/modules/memory.md)
   - [会话管理](zh/modules/session.md)
   - [RAG](zh/modules/rag.md)
   - [工作空间](zh/modules/workspace.md)
   - [MCP 集成](zh/modules/mcp.md)
   - [技能](zh/modules/skill.md)
   - [沙箱](zh/modules/sandbox.md)
3. [Python → Rust 迁移参考](zh/migration.md)
4. [场景教程：RAG 知识库问答](zh/tutorials/rag-knowledge-chat.md)

---

## English Documentation

**Entry point**: [Getting Started](en/getting-started.md) — run your first agent in 30 minutes

**Recommended reading order**:

1. [Getting Started](en/getting-started.md)
2. Module docs ([modules/](en/modules/)):
   - [Messages & Core Types](en/modules/message-types.md)
   - [Events & Streaming](en/modules/event-streaming.md)
   - [Model Abstraction](en/modules/model.md)
   - [DashScope Provider](en/modules/dashscope.md)
   - [Tool System](en/modules/tool.md)
   - [Agent System](en/modules/agent.md)
   - [Memory](en/modules/memory.md)
   - [Session Management](en/modules/session.md)
   - [RAG](en/modules/rag.md)
   - [Workspace](en/modules/workspace.md)
   - [MCP Integration](en/modules/mcp.md)
   - [Skill](en/modules/skill.md)
   - [Sandbox](en/modules/sandbox.md)
3. [Python → Rust Migration Guide](en/migration.md)
4. [Tutorial: RAG Knowledge-Base Chat](en/tutorials/rag-knowledge-chat.md)

---

## 规划中 / Planned

以下模块在上游路线图中但尚未在 Rust 侧实现，暂无使用文档：

The following modules exist in the upstream roadmap but are not yet implemented in Rust — no usage documentation yet:

- Multi-agent 多智能体协作 / Multi-agent collaboration
- Distributed runtime 分布式运行时 / Distributed runtime
