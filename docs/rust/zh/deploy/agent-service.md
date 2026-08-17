---
title: "智能体即服务"
description: "把智能体托管为多租户多会话 HTTP 服务"
---

<Note>
**Rust 实现状态**: 计划中。多租户、多会话的 HTTP 服务层（负责请求路由、会话状态、持久化、调度与工具卸载）在 AgentScope Rust 中尚未实现。当前可在 Rust 侧自建 HTTP 服务（axum/actix）并把 `ReActAgent` 嵌入其中；库级 Agent 能力（reply / reply_stream / 状态持久化）见 [Agent 概述](../building-blocks/agent/overview)
</Note>

# 智能体即服务（计划中）

## 能力概述

「智能体即服务」是把智能体从「进程内库」提升为「网络服务」的部署形态：多个用户或会话通过 HTTP 访问同一个智能体服务，服务层负责请求路由、会话状态隔离、持久化、调度与工具卸载。它解决的是**规模化访问**问题——单进程内跑一个 `ReActAgent` 只服务当前进程，而服务化让智能体可以被多个客户端并发使用。

典型形态：

- **REST 端点**：客户端发起一次对话请求，拿到完整回复；
- **SSE 会话流**：按流式事件推送（对应库内的 `reply_stream` 事件）；
- **后台任务 / 调度**：定时触发、长任务异步执行；
- **前端 schema 驱动**：前端基于模型 schema 动态渲染界面。

## Rust 缺失范围

Rust 当前无 HTTP server（无 axum/actix 等依赖），因此 agent-service 相关的全部能力（REST 端点、SSE 会话流、后台任务、cron 调度、前端 schema 驱动）均未实现

## 替代能力

当前可在 Rust 侧自建 HTTP 服务（axum/actix）并把 `ReActAgent` 嵌入其中；库级 Agent 能力（reply / reply_stream / 状态持久化）见 [Agent 概述](../building-blocks/agent/overview)

## 相关

- 库级基础能力见对应的 building-blocks 页面。
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
