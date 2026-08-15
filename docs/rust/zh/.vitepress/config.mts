import { defineConfig } from 'vitepress'
import { sidebar } from './sidebar.mts'

export default defineConfig({
  lang: 'zh-CN',
  title: 'AgentScope Rust',
  description: '面向 Rust 的智能体开发框架',
  base: '/agentscope-rust/',
  cleanUrls: true,
  ignoreDeadLinks: false,
  // 高对比度代码主题：默认 github-light/dark 的若干 token 与 .lang 标签
  // 无法满足 WCAG AA 对比度（axe color-contrast serious）。
  markdown: {
    theme: {
      light: 'github-light-high-contrast',
      dark: 'github-dark-high-contrast'
    }
  },
  themeConfig: {
    nav: [
      { text: '首页', link: '/' },
      { text: '快速开始', link: '/quickstart' },
      { text: '构建模块', link: '/building-blocks/agent/overview' },
      { text: '部署与集成', link: '/deploy/agent-service' },
      { text: 'FAQ', link: '/others/faq' },
      { text: 'GitHub', link: 'https://github.com/NingNing0111/agentscope-rust' }
    ],
    sidebar,
    search: { provider: 'local' },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/NingNing0111/agentscope-rust' }
    ]
  }
})
