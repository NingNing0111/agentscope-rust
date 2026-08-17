---
title: "飞书"
description: "飞书渠道接入"
---

<Note>
**Rust 实现状态**: 计划中。飞书（Feishu）渠道实现在 AgentScope Rust 中尚未实现。
</Note>

# 飞书（计划中）

## 能力概述

飞书渠道把智能体接入飞书（群聊、单聊与机器人卡片），让同事在飞书里直接提问并收到智能体回复。典型用途包括：企业知识问答、内部流程助手、群机器人值班。

接入后，飞书里的消息会成为智能体的输入，智能体的回复会以机器人消息（含卡片）形式发回对应会话。

## Rust 缺失范围

Rust 当前无 channel 相关代码（无飞书/Discord 等 IM 接入）。

## 替代能力

暂无替代。可在 Rust 侧自行对接 IM 平台 Webhook / SDK 并把消息转成 `Msg` 送入 `ReActAgent`（见 [Agent 概述](../../building-blocks/agent/overview)）。

## 相关

- [渠道概述](overview)、[自定义渠道](custom)、[Discord](discord)、[路由规则](routing)
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
