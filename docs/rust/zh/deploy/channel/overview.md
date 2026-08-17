---
title: "渠道概述"
description: "通过 IM 平台与智能体交互"
---

<Note>
**Rust 实现状态**: 计划中。channel 模块把 agent-service 中的智能体连接到 IM 平台，在 AgentScope Rust 中尚未实现。
</Note>

# 渠道概述（计划中）

## 能力概述

渠道（Channel）是把智能体「接上」即时通讯（IM）平台的桥：用户在你的飞书或 Discord 群里发消息，消息经过渠道转成智能体能读懂的 `Msg`，智能体的回复再由渠道发回群里。这样一来，使用者不必打开专门的网页或终端，在平时用的聊天软件里就能直接对话。

渠道层要解决的核心问题是**平台差异的屏蔽**：

- `ChannelBase` 抽象出「一个 IM 平台」的通用行为（接收消息、发送消息、处理会话）；
- 内置 `FeishuChannel` / `DiscordChannel` 等实现，各自封装对应平台的接入细节；
- `ChannelGateway` 统一协调每个入站事件，把不同平台的请求转成一致的内部消息流。

典型工作流：**IM 收到消息 → 渠道把消息转成 `Msg` → 送入智能体 → 智能体回复 → 渠道把回复发回 IM**。

## Rust 缺失范围

Rust 当前无 channel 相关代码（无飞书/Discord 等 IM 接入）。

## 替代能力

暂无替代。可在 Rust 侧自行对接 IM 平台 Webhook / SDK 并把消息转成 `Msg` 送入 `ReActAgent`（见 [Agent 概述](../../building-blocks/agent/overview)）。

## 相关

- [自定义渠道](custom)、[飞书](feishu)、[Discord](discord)、[路由规则](routing)
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
