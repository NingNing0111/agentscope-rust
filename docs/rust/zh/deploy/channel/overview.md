---
title: "渠道概述"
description: "通过 IM 平台与智能体交互"
---

<Note>
**Rust 实现状态**: 计划中。channel 模块把 agent-service 中的智能体连接到 IM 平台，在 AgentScope Rust 中尚未实现。
</Note>

# 渠道概述（计划中）

## 能力概述

channel 模块是 AgentScope 连接即时通讯平台的方式，`ChannelBase` 抽象平台，内置 `FeishuChannel` / `DiscordChannel` 实现，`ChannelGateway` 协调每个入站事件。

## Rust 缺失范围

Rust 当前无 channel 相关代码（无飞书/Discord 等 IM 接入）。

## 替代能力

暂无替代。可在 Rust 侧自行对接 IM 平台 Webhook / SDK 并把消息转成 `Msg` 送入 `ReActAgent`（见 [Agent 概述](../../building-blocks/agent/overview)）。

## 相关

- [自定义渠道](custom)、[飞书](feishu)、[Discord](discord)、[路由规则](routing)
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
