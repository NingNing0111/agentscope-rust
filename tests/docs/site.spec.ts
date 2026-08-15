import { expect, test } from '@playwright/test'

test.describe('homepage smoke', () => {
  test('hero heading, quickstart / GitHub links and status text are present', async ({ page }) => {
    await page.goto('/agentscope-rust/')

    // page title and hero heading include "AgentScope Rust"
    await expect(page).toHaveTitle(/AgentScope Rust/)
    await expect(page.getByRole('heading', { name: /AgentScope Rust/ })).toBeVisible()

    // visible 快速开始 link resolves under /agentscope-rust/quickstart
    const quickstart = page.getByRole('link', { name: '快速开始' }).first()
    await expect(quickstart).toBeVisible()
    await expect(quickstart).toHaveAttribute('href', /\/agentscope-rust\/quickstart/)

    // GitHub link points to the repository
    const github = page.getByRole('link', { name: 'GitHub 仓库' })
    await expect(github).toBeVisible()
    await expect(github).toHaveAttribute(
      'href',
      'https://github.com/NingNing0111/agentscope-rust'
    )

    // status vocabulary visible
    for (const word of ['实现状态', '已实现', '部分支持', '计划中']) {
      await expect(page.getByText(word).first()).toBeVisible()
    }
  })

  test('card link navigates via keyboard', async ({ page }) => {
    await page.goto('/agentscope-rust/')

    const card = page.getByRole('link', { name: /Agent \/ 事件/ })
    await card.focus()
    await expect(card).toBeFocused()

    await page.keyboard.press('Enter')
    await expect(page).toHaveURL(
      /\/agentscope-rust\/building-blocks\/agent\/overview\/?$/
    )
    // the overview page renders its first h2 核心接口 (the site's doc pages
    // have no h1: VitePress only emits an h1 from a markdown `#` heading)
    await expect(
      page.getByRole('heading', { name: /核心接口/ })
    ).toBeVisible()
  })
})

test.describe('navigation', () => {
  test('deep route loads directly and survives reload', async ({ page }) => {
    await page.goto('/agentscope-rust/building-blocks/agent/overview')
    await expect(page).toHaveURL(
      /\/agentscope-rust\/building-blocks\/agent\/overview\/?$/
    )
    await expect(
      page.getByRole('heading', { name: /核心接口/ })
    ).toBeVisible()

    await page.reload()
    await expect(page.getByRole('heading', { name: /核心接口/ })).toBeVisible()
    await expect(page).toHaveURL(
      /\/agentscope-rust\/building-blocks\/agent\/overview\/?$/
    )
  })

  test('desktop top nav opens quickstart', async ({ page, isMobile }) => {
    test.skip(isMobile, 'top nav is hidden on mobile viewports')
    await page.goto('/agentscope-rust/')

    const nav = page.getByRole('navigation', { name: 'Main Navigation' })
    await nav.getByRole('link', { name: '快速开始' }).click()

    await expect(page).toHaveURL(/\/agentscope-rust\/quickstart\/?$/)
    await expect(
      page.getByRole('heading', { name: /环境准备/ })
    ).toBeVisible()
  })

  test('mobile menu exposes quickstart and FAQ', async ({ page, isMobile }) => {
    test.skip(!isMobile, 'hamburger menu is only used on mobile viewports')
    await page.goto('/agentscope-rust/')

    await page.getByRole('button', { name: 'mobile navigation' }).click()

    const screen = page.locator('#VPNavScreen')
    await expect(screen.getByRole('link', { name: '快速开始' })).toBeVisible()
    await expect(screen.getByRole('link', { name: 'FAQ' })).toBeVisible()
  })

  test('sidebar link navigates to a deep page', async ({ page, isMobile }) => {
    test.skip(isMobile, 'sidebar is hidden on mobile viewports')
    await page.goto('/agentscope-rust/building-blocks/agent/overview')

    const sidebar = page.getByRole('navigation', { name: 'Sidebar Navigation' })
    await sidebar.getByRole('link', { name: '长期记忆' }).click()

    await expect(page).toHaveURL(
      /\/agentscope-rust\/building-blocks\/long-term-memory\/?$/
    )
    await expect(
      page.getByRole('heading', { name: /Memory trait/ })
    ).toBeVisible()
  })
})

test.describe('local search', () => {
  test('opens via "/" and finds a known page title', async ({ page }) => {
    await page.goto('/agentscope-rust/')

    // wait for hydration so the "/" hotkey listener is registered
    await expect(page.getByRole('button', { name: /search/i })).toBeVisible()

    await page.keyboard.press('/')
    const input = page.getByRole('searchbox')
    await expect(input).toBeVisible()

    await input.fill('人机交互')
    // VitePress local search indexes section headings + their text, not
    // frontmatter titles, so the query 人机交互 returns section-level hits
    // from pages that discuss human-in-the-loop (e.g. permission-system
    // overview's 决策流程 section) rather than an option literally named
    // 人机交互.
    await expect(page.getByRole('option').first()).toBeVisible()
  })
})

test.describe('dark mode', () => {
  test('toggle flips the root class and persists after reload', async ({
    page,
    isMobile
  }) => {
    await page.goto('/agentscope-rust/')

    if (isMobile) {
      await page.getByRole('button', { name: 'mobile navigation' }).click()
    }
    const toggle = isMobile
      ? page.locator('#VPNavScreen').getByRole('switch')
      : page.getByRole('switch').first()
    await expect(toggle).toBeVisible()

    const isDark = () =>
      page.evaluate(() =>
        document.documentElement.classList.contains('dark')
      )
    const initialDark = await isDark()

    await toggle.click()
    await expect.poll(isDark).toBe(!initialDark)

    await page.reload()
    await expect.poll(isDark).toBe(!initialDark)
  })
})

test.describe('accordion keyboard interaction', () => {
  test('Enter expands, Space collapses', async ({ page }) => {
    await page.goto('/agentscope-rust/others/faq')

    const button = page.getByRole('button', {
      name: 'AgentScope Rust 是什么？'
    })
    await button.focus()
    await expect(button).toBeFocused()
    await expect(button).toHaveAttribute('aria-expanded', 'false')

    await page.keyboard.press('Enter')
    await expect(button).toHaveAttribute('aria-expanded', 'true')
    const panelId = await button.getAttribute('aria-controls')
    const panel = page.locator(`#${panelId}`)
    await expect(panel).toBeVisible()

    await page.keyboard.press('Space')
    await expect(button).toHaveAttribute('aria-expanded', 'false')
    await expect(panel).toBeHidden()
  })
})
