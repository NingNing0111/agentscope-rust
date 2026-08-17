---
title: "自定义渠道"
description: "自定义 IM 渠道接入"
---

<Note>
**Rust 实现状态**: 计划中。自定义 channel 接入方式在 AgentScope Rust 中尚未实现。
</Note>

# 自定义渠道（计划中）

## 能力概述

内置渠道覆盖不了所有平台，所以渠道层提供自定义入口：对没有现成实现的 IM（或内部通讯系统），开发者只需实现渠道抽象定义的「如何收消息、如何发消息」，即可接入。

自定义渠道要解决的典型场景：

- 接入内部 IM 或自研通讯工具；
- 对接有特殊鉴权 / 消息格式的平台；
- 在官方渠道基础上定制消息预处理（如过滤、改写、附加上下文）。

## Rust 缺失范围

Rust 当前无 channel 相关代码（无飞书/Discord 等 IM 接入）。

## 替代能力

暂无替代。可在 Rust 侧自行对接 IM 平台 Webhook / SDK 并把消息转成 `Msg` 送入 `ReActAgent`（见 [Agent 概述](../../building-blocks/agent/overview)）。

## 相关

- [渠道概述](overview)、[飞书](feishu)、[Discord](discord)、[路由规则](routing)
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
