---
title: "Hub 概述"
description: "浏览与安装外部 MCP 与技能注册表"
---

<Note>
**Rust 实现状态**: 计划中。hub 模块浏览外部注册表（MCP hub / skill hub）并把卡片写入用户库，在 AgentScope Rust 中尚未实现。库级 MCP 客户端配置（`McpRegistry`）与技能管理（`SkillManager`）见 [工作空间](../../building-blocks/workspace/manage-resources)
</Note>

# Hub 概述（计划中）

## 能力概述

Hub 是「发现能力」的入口：开发者浏览一个外部注册表，找到想要的 MCP 服务或技能，一键安装到自己的用户库中，之后即可在智能体里使用。它解决的是**能力发现与分发**问题——不用手写连接配置或复制技能目录，点选即可接入。

Hub 通常与 [agent-service](agent-service) 配套：服务层负责托管智能体，Hub 负责让智能体更容易获得新工具与技能。可分为两类：**MCP hub**（安装 MCP 服务）与 **skill hub**（安装技能）。

## Rust 缺失范围

Rust 当前无 hub 服务代码。

## 替代能力

库级 MCP 客户端配置（`McpRegistry`）与技能管理（`SkillManager`）见 [工作空间](../../building-blocks/workspace/manage-resources)

## 相关

- [MCP Hub](mcp-hub)、[技能 Hub](skill-hub)
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
