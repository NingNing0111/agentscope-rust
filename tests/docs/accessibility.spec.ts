import { expect, test } from '@playwright/test'
import AxeBuilder from '@axe-core/playwright'

// baseURL is http://127.0.0.1:4173/agentscope-rust/, so root-relative goto()
// would drop the base; use full paths under the base instead.
const scannedPages = [
  '/agentscope-rust/',
  '/agentscope-rust/others/faq',
  '/agentscope-rust/building-blocks/agent/overview'
] as const

for (const path of scannedPages) {
  test(`axe: no serious or critical violations on ${path}`, async ({ page }) => {
    await page.goto(path)

    // FAQ: exercise one accordion before scanning so its open state is included
    if (path.endsWith('/others/faq')) {
      await page
        .getByRole('button', { name: 'AgentScope Rust 是什么？' })
        .click()
    }

    const results = await new AxeBuilder({ page }).analyze()
    const seriousOrCritical = results.violations.filter(
      (v) => v.impact === 'serious' || v.impact === 'critical'
    )

    expect(
      seriousOrCritical,
      `serious/critical violations on ${path}: ${JSON.stringify(
        seriousOrCritical.map((v) => ({
          id: v.id,
          impact: v.impact,
          help: v.help,
          nodes: v.nodes.length
        })),
        null,
        2
      )}`
    ).toEqual([])
  })
}
