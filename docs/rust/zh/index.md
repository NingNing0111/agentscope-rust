---
layout: home

hero:
  name: AgentScope Rust
  text: 面向 Rust 的智能体开发框架
  tagline: 用可组合的 Agent、Tool、Memory、RAG 与 Workspace 构建可控的智能体应用
  actions:
    - theme: brand
      text: 快速开始
      link: /quickstart
    - theme: alt
      text: 查看模块
      link: /building-blocks/agent/overview
    - theme: alt
      text: GitHub 仓库
      link: https://github.com/NingNing0111/agentscope-rust
---

AgentScope Rust（`agent_scope_*` crates）是 **AgentScope 的 Rust 重构版**。它以 Rust 多 crate workspace 组织，覆盖智能体应用的关键环节：Agent 编排、工具系统、长期记忆、RAG、工作空间、沙箱、权限与事件驱动流式输出。

文档会明确区分「已实现」「部分支持」「计划中」。尚未实现的能力不会伪装成可用 API；对应页面会说明当前边界，并给出可替代的 Rust 能力或迁移提示。

## 30 秒路线

<CardGroup :cols="3">
	<Card title="跑起来" icon="1" href="/quickstart" cta="从 Quickstart 开始">
		配置模型凭据，运行 `examples/quickstart`，观察 `reply` 与 `reply_stream` 的最小闭环。
	</Card>
	<Card title="接工具" icon="2" href="/building-blocks/tool/overview" cta="理解 ToolKit">
		注册 FunctionTool、MCP Tool 或 Skill，让 Agent 能调用外部能力。
	</Card>
	<Card title="加边界" icon="3" href="/building-blocks/permission-system/overview" cta="配置权限">
		用权限模式、规则和 HITL 审核控制工具调用与工作空间操作。
	</Card>
</CardGroup>

## 核心能力

<CardGroup :cols="2">
	<Card title="Agent / 事件" icon="🤖" href="/building-blocks/agent/overview" cta="立即上手">
		自主 ReAct 推理-行动循环，支持流式事件、人机协同审核、中断与状态持久化。
	</Card>
	<Card title="Tool / MCP" icon="🧰" href="/building-blocks/tool/overview" cta="了解工具系统">
		函数工具（FunctionTool）与工具注册表（ToolKit），支持 MCP 客户端接入与 Skill 技能加载。
	</Card>
	<Card title="Memory / RAG" icon="🧠" href="/building-blocks/long-term-memory" cta="查看记忆">
		文件型与向量型双后端长期记忆，KnowledgeBase 与 RAG 中间件。
	</Card>
	<Card title="Workspace / Sandbox" icon="📦" href="/building-blocks/workspace/overview" cta="了解工作空间">
		本地隔离工作空间（LocalWorkspace）与沙箱，文件操作与命令执行边界受控。
	</Card>
	<Card title="权限 / HITL" icon="🛡️" href="/building-blocks/permission-system/overview" cta="了解权限系统">
		allow / deny / ask 权限引擎，人机协同确认（Human-in-the-Loop）与工具内置检查。
	</Card>
	<Card title="Skill / SubAgent / 任务规划" icon="🗂️" href="/building-blocks/plan" cta="了解任务规划">
		Skill 技能系统、SubAgent 委派与任务规划工具（Plan）。
	</Card>
</CardGroup>

## 实现状态

每个模块页面顶部都有「Rust 实现状态」标注，三档取值保证内容真实可用、不伪造兼容能力：

- **已实现** — 能力在 AgentScope Rust 中可用，文档内容为真实 Rust 用法；
- **部分支持** — 部分可用，页面列出「已支持 / 尚未实现」边界；
- **计划中** — Rust 尚未实现，页面说明缺失范围与替代能力，无 Rust 代码。

服务化部署能力（agent-service、channel、hub 等）当前多为「计划中」，各模块页面会直接标明对应能力的真实实现状态。

## 常用入口

| 目标 | 入口 |
| --- | --- |
| 运行第一个 Agent | [快速开始](/quickstart) |
| 理解 ReActAgent 主循环 | [Agent 概述](/building-blocks/agent/overview) |
| 注册和管理工具 | [Tool 概述](/building-blocks/tool/overview) |
| 接入 MCP 工具 | [MCP](/building-blocks/tool/mcp) |
| 持久化记忆与检索 | [长期记忆](/building-blocks/long-term-memory) / [RAG](/building-blocks/rag) |
| 控制工具权限 | [权限系统](/building-blocks/permission-system/overview) |

## 快速上手

<Note>
30 分钟跑起第一个可对话的 Agent，见 [快速开始](/quickstart)。真实模型调用需要环境变量 `DEFAULT_API_KEY`；无凭据时示例会给出明确错误提示。
</Note>
