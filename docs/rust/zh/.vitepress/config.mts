import { defineConfig } from 'vitepress'
import { sidebar } from './sidebar.mts'

export default defineConfig({
  lang: 'zh-CN',
  title: 'AgentScope Rust',
  description: '面向 Rust 的智能体开发框架',
  base: '/agentscope-rust/',
  cleanUrls: true,
  ignoreDeadLinks: false,
  themeConfig: {
    nav: [
      { text: '首页', link: '/' },
      { text: 'Agent', link: '/building-blocks/agent/overview' },
      { text: 'FAQ', link: '/others/faq' }
    ],
    sidebar,
    search: { provider: 'local' },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/NingNing0111/agentscope-rust' }
    ]
  }
})
