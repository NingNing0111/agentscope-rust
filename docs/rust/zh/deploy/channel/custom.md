---
title: "自定义渠道"
description: "自定义 IM 渠道接入"
---

<Note>
**Rust 实现状态**: 计划中。自定义 channel 接入方式在 AgentScope Rust 中尚未实现。
</Note>

# 自定义渠道（计划中）

## 能力概述

自定义 channel 接入方式，是 AgentScope 连接即时通讯平台的方式。

## Rust 缺失范围

Rust 当前无 channel 相关代码（无飞书/Discord 等 IM 接入）。

## 替代能力

暂无替代。可在 Rust 侧自行对接 IM 平台 Webhook / SDK 并把消息转成 `Msg` 送入 `ReActAgent`（见 [Agent 概述](../../building-blocks/agent/overview)）。

## 相关

各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
