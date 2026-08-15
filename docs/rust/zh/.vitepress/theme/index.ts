import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import Note from './components/Note.vue'
import Tip from './components/Tip.vue'
import Card from './components/Card.vue'
import CardGroup from './components/CardGroup.vue'
import Badge from './components/Badge.vue'
import Accordion from './components/Accordion.vue'
import AccordionGroup from './components/AccordionGroup.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('Note', Note)
    app.component('Tip', Tip)
    app.component('Card', Card)
    app.component('CardGroup', CardGroup)
    app.component('Badge', Badge)
    app.component('Accordion', Accordion)
    app.component('AccordionGroup', AccordionGroup)

    // VitePress 1.6.4 上游缺陷：VPSwitchAppearance 的 title 属性没有转发到
    // VPSwitch 按钮（VPSwitch.vue 未绑定 :title），导致主题切换按钮无
    // 无障碍名称（axe button-name critical）。这里在客户端为切换按钮补齐
    // aria-label，保证 WCAG AA 无 serious/critical 问题。
    if (typeof window !== 'undefined') {
      const labelSwitch = () => {
        for (const button of document.querySelectorAll<HTMLButtonElement>(
          'button.VPSwitchAppearance'
        )) {
          if (!button.getAttribute('aria-label')) {
            button.setAttribute('aria-label', '切换主题')
          }
        }
      }
      labelSwitch()
      const observer = new MutationObserver(labelSwitch)
      observer.observe(document.body, { childList: true, subtree: true })
    }
  }
} satisfies Theme
