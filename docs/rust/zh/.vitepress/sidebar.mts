import type { DefaultTheme } from 'vitepress'

/**
 * 显式维护的 50 页侧边栏。页面集合必须与 docs/rust/mirror-map.md
 * 以及 docs/rust/zh 目录下全部正式 .md 页面完全相等（由 npm run docs:check 强制）。
 */
export const sidebar: DefaultTheme.Sidebar = [
  {
    text: '开始使用',
    items: [
      { text: '首页', link: '/' },
      { text: '快速开始', link: '/quickstart' },
      { text: '发布说明', link: '/release-notes' }
    ]
  },
  {
    text: 'Agent',
    items: [
      { text: '概述', link: '/building-blocks/agent/overview' },
      { text: '配置智能体', link: '/building-blocks/agent/configure-agent' },
      { text: '运行智能体', link: '/building-blocks/agent/run-agent' },
      { text: '人机交互', link: '/building-blocks/agent/human-in-the-loop' },
      { text: '中断智能体', link: '/building-blocks/agent/interrupt-agent' }
    ]
  },
  {
    text: 'Context',
    items: [
      { text: '概述', link: '/building-blocks/context/overview' },
      { text: '压缩上下文', link: '/building-blocks/context/compress-context' },
      { text: '感知环境', link: '/building-blocks/context/environment-awareness' },
      { text: '卸载上下文', link: '/building-blocks/context/offload-context' }
    ]
  },
  {
    text: 'Model',
    items: [
      { text: '模型概览', link: '/building-blocks/model/overview' },
      { text: '大语言模型', link: '/building-blocks/model/llm' },
      { text: '嵌入模型', link: '/building-blocks/model/embedding' },
      { text: '语音合成', link: '/building-blocks/model/tts' }
    ]
  },
  {
    text: 'Tool',
    items: [
      { text: '概述', link: '/building-blocks/tool/overview' },
      { text: '函数工具', link: '/building-blocks/tool/python-tool' },
      { text: 'MCP', link: '/building-blocks/tool/mcp' },
      { text: 'Skill', link: '/building-blocks/tool/skill' },
      { text: '元工具', link: '/building-blocks/tool/manage-tools' }
    ]
  },
  {
    text: 'Workspace',
    items: [
      { text: '概述', link: '/building-blocks/workspace/overview' },
      { text: '运行工作空间', link: '/building-blocks/workspace/run-workspace' },
      { text: '管理资源', link: '/building-blocks/workspace/manage-resources' },
      { text: 'MCP Gateway', link: '/building-blocks/workspace/mcp-gateway' }
    ]
  },
  {
    text: '构建模块',
    items: [
      { text: '消息与事件', link: '/building-blocks/message-and-event' },
      { text: '长期记忆', link: '/building-blocks/long-term-memory' },
      { text: '中间件', link: '/building-blocks/middleware' },
      {
        text: '权限系统',
        items: [
          { text: '概述', link: '/building-blocks/permission-system/overview' },
          { text: '权限模式', link: '/building-blocks/permission-system/permission-mode' },
          { text: '权限规则', link: '/building-blocks/permission-system/permission-rule' },
          { text: '工具内置检查', link: '/building-blocks/permission-system/tool-check' }
        ]
      },
      { text: '计划模式', link: '/building-blocks/plan' },
      { text: 'RAG', link: '/building-blocks/rag' },
      { text: '控制台', link: '/building-blocks/console' }
    ]
  },
  {
    text: '部署与集成',
    items: [
      { text: '智能体即服务', link: '/deploy/agent-service' },
      { text: '智能体团队', link: '/deploy/agent-team' },
      {
        text: '渠道（Channel）',
        items: [
          { text: '渠道概述', link: '/deploy/channel/overview' },
          { text: '路由规则', link: '/deploy/channel/routing' },
          { text: 'Discord', link: '/deploy/channel/discord' },
          { text: '飞书', link: '/deploy/channel/feishu' },
          { text: '自定义渠道', link: '/deploy/channel/custom' }
        ]
      },
      {
        text: 'Hub',
        items: [
          { text: 'Hub 概述', link: '/deploy/hub/overview' },
          { text: 'MCP Hub', link: '/deploy/hub/mcp-hub' },
          { text: '技能 Hub', link: '/deploy/hub/skill-hub' }
        ]
      },
      { text: 'RAG 服务', link: '/deploy/rag' },
      { text: '分享与发布', link: '/deploy/sharing' },
      { text: '工作空间管理', link: '/deploy/workspace-manager' }
    ]
  },
  {
    text: '其他',
    items: [
      { text: '版本迁移', link: '/others/change-log' },
      { text: '常见问题', link: '/others/faq' }
    ]
  }
]
