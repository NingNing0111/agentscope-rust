import type { DefaultTheme } from 'vitepress'

export const sidebar: DefaultTheme.Sidebar = [
  {
    text: '原型页面',
    items: [
      { text: '首页', link: '/' },
      { text: 'Agent 概览', link: '/building-blocks/agent/overview' },
      { text: '变更说明', link: '/others/change-log' },
      { text: '常见问题', link: '/others/faq' }
    ]
  }
]
