---
title: "路由规则"
description: "渠道消息路由"
---

<Note>
**Rust 实现状态**: 计划中。channel 消息路由与生命周期协调在 AgentScope Rust 中尚未实现。
</Note>

# 路由规则（计划中）

## 能力概述

当多个渠道（飞书、Discord、自定义）同时接入时，需要明确「一条消息该去哪、由谁处理」。路由规则负责两件事：

- **入站路由**：根据会话 / 用户 / 内容把消息分发到正确的智能体或会话；
- **生命周期协调**：管理长会话的状态与回复归属，避免多个渠道互相干扰。

没有路由规则，多平台接入会出现消息串线、回复错位等问题。路由层把「平台无关」的消息流统一收敛，交给同一个智能体处理。

## Rust 缺失范围

Rust 当前无 channel 相关代码（无飞书/Discord 等 IM 接入）。

## 替代能力

暂无替代。可在 Rust 侧自行对接 IM 平台 Webhook / SDK 并把消息转成 `Msg` 送入 `ReActAgent`（见 [Agent 概述](../../building-blocks/agent/overview)）。

## 相关

- [渠道概述](overview)、[自定义渠道](custom)、[飞书](feishu)、[Discord](discord)
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
