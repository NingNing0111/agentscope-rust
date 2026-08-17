---
title: "MCP Hub"
description: "MCP 服务注册表"
---

<Note>
**Rust 实现状态**: 计划中。MCP hub（浏览并安装外部 MCP server 到用户库）在 AgentScope Rust 中尚未实现。库级 MCP 客户端接入见 [tool/mcp](../../building-blocks/tool/mcp)
</Note>

# MCP Hub（计划中）

## 能力概述

MCP Hub 是 MCP 服务的注册与分发中心：开发者浏览一个目录，找到可复用的 MCP 服务（例如代码搜索、数据库、绘图服务），一键安装后即可在智能体中作为工具使用。它把「手动配置 MCP 连接」变成「点选安装」，降低接入成本。

## Rust 缺失范围

Rust 当前无 hub 服务代码。

## 替代能力

库级 MCP 客户端接入见 [tool/mcp](../../building-blocks/tool/mcp)

## 相关

- [Hub 概述](overview)、[技能 Hub](skill-hub)
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
