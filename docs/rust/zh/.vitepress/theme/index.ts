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
  }
} satisfies Theme
