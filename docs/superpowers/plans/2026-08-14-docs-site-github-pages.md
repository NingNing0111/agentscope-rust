# AgentScope Rust 中文文档站实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `docs/rust/zh/` 的 50 个 Mintlify 风格 `.mdx` 页面迁移为可由 VitePress 1.6.4 构建和验证的中文文档站，并通过 GitHub Actions 自动发布到 GitHub Pages。

**Architecture:** 先用四个代表页面验证 VitePress、Vue 兼容组件和路由方案，再原地迁移全部 50 页。仓库内 Node 检查器负责页面集合、组件、路由、源码链接和构建产物边界；Playwright/axe 负责浏览器行为和无障碍；独立 GitHub Actions workflow 将只读 PR 构建与有权限的 Pages 部署隔离。

**Tech Stack:** Node.js 22、npm、VitePress 1.6.4、Vue 3、Node.js test runner、Playwright、`@axe-core/playwright`、GitHub Actions、GitHub Pages

**Spec:** `docs/superpowers/specs/2026-08-14-docs-site-github-pages-design.md`

## Global Constraints

- 仅发布 `docs/rust/zh/` 中文 Rust 文档，不发布 `docs/python/`。
- Node.js 固定主版本 `22`；VitePress 精确固定为 `1.6.4`。
- 正式页面必须从 50 个 `.mdx` 原地迁移为 50 个 `.md`；最终正式 `.mdx` 数量为零。
- `docs/rust/mirror-map.md`、文件系统和 VitePress sidebar 的页面集合必须完全相等。
- 组件兼容范围固定为 `Note`、`Tip`、`Card`、`CardGroup`、`Badge`、`Accordion`、`AccordionGroup`。
- 未登记组件和关键属性必须 fail closed；组件扫描必须忽略 fenced code 和 inline code。
- `cols={2}` 必须迁移为 Vue 模板语法 `:cols="2"`。
- 内容路由不得残留 `/versions/0.1.0/zh/`，不得硬编码 `/agentscope-rust/`。
- 生产 `base` 固定为 `/agentscope-rust/`。
- 内容根外的 `examples/**`、`crates/**` 链接改为跟随 `master` 的 GitHub URL；维护文件不公开。
- workflow 顶层仅 `contents: read`；只有 `deploy` job 拥有 `pages: write` 与 `id-token: write`。
- PR 只检查和构建；`master` push 与 `workflow_dispatch` 才可部署。
- 所有仓库命令都使用 `rtk` 前缀。
- 阶段 1 四页原型未通过前，不得执行剩余 46 页迁移。

---

## 文件职责映射

### Node/VitePress 工具链

- `package.json`：固定依赖并公开 `docs:*` 和 `test:docs` scripts。
- `package-lock.json`：锁定可复现依赖树。
- `.nvmrc`：固定 Node.js 22。
- `docs/rust/zh/.vitepress/config.mts`：站点 `base`、nav、sidebar、搜索、social links、dead-link 策略。
- `docs/rust/zh/.vitepress/sidebar.mts`：只保存 50 页 sidebar 数据，并导出给配置和检查器复用。
- `docs/rust/zh/.vitepress/theme/index.ts`：注册七个全局兼容组件并加载主题样式。
- `docs/rust/zh/.vitepress/theme/custom.css`：首页、组件、明暗主题和移动端样式。
- `docs/rust/zh/.vitepress/theme/components/*.vue`：每个文件只实现一个 Mintlify 兼容组件。

### 文档检查

- `scripts/docs/lib/docs-model.mjs`：解析 mirror map、枚举页面、标准化路由、提取 sidebar 路由。
- `scripts/docs/lib/markdown-scan.mjs`：剥离代码块后提取组件、属性、Markdown 链接和组件 `href`。
- `scripts/docs/check-docs.mjs`：组合静态规则并以非零退出码报告失败。
- `scripts/docs/check-built-site.mjs`：验证 dist HTML、站内目标、base path 和禁止内容。
- `scripts/docs/check-examples.mjs`：提取公开文档引用的仓库路径与 `cargo run/check -p` package，并验证其存在。
- `tests/docs/docs-model.test.mjs`：页面集合和路由规范单元测试。
- `tests/docs/markdown-scan.test.mjs`：组件、属性、代码块过滤和链接提取单元测试。
- `tests/docs/check-built-site.test.mjs`：构建产物检查器 fixture 测试。
- `tests/docs/check-examples.test.mjs`：示例 package/路径检查器 fixture 测试。

### 浏览器验证

- `playwright.config.mts`：生产 preview webServer、桌面/移动项目与 `/agentscope-rust/` baseURL。
- `tests/docs/site.spec.ts`：首页、深层路由、nav/sidebar、搜索、暗色模式、Accordion/Card smoke。
- `tests/docs/accessibility.spec.ts`：首页、FAQ、深层页 axe 检查。

### 发布与维护文档

- `.github/workflows/docs.yml`：只读 build job 与最小权限 deploy job。
- `docs/rust/README.md`：新站路由和本地开发命令。
- `docs/rust/mirror-map.md`：50 个 Rust 页面扩展名和新路由政策。
- `scripts/check-docs-mirror.sh`：从 `.mdx` 假设更新为 `.md`，继续验证 Python/Rust 镜像与状态块。
- `README.md`：加入中文文档站入口。

---

### Task 1: 建立 Node 工具链与四页 VitePress 技术原型

**Files:**
- Create: `.nvmrc`
- Create: `package.json`
- Create: `package-lock.json`
- Create: `docs/rust/zh/.vitepress/config.mts`
- Create: `docs/rust/zh/.vitepress/sidebar.mts`
- Create: `docs/rust/zh/.vitepress/theme/index.ts`
- Create: `docs/rust/zh/.vitepress/theme/custom.css`
- Create: `docs/rust/zh/.vitepress/theme/components/Note.vue`
- Create: `docs/rust/zh/.vitepress/theme/components/Tip.vue`
- Create: `docs/rust/zh/.vitepress/theme/components/Card.vue`
- Create: `docs/rust/zh/.vitepress/theme/components/CardGroup.vue`
- Create: `docs/rust/zh/.vitepress/theme/components/Badge.vue`
- Create: `docs/rust/zh/.vitepress/theme/components/Accordion.vue`
- Create: `docs/rust/zh/.vitepress/theme/components/AccordionGroup.vue`
- Rename: `docs/rust/zh/index.mdx` → `docs/rust/zh/index.md`
- Rename: `docs/rust/zh/others/faq.mdx` → `docs/rust/zh/others/faq.md`
- Rename: `docs/rust/zh/others/change-log.mdx` → `docs/rust/zh/others/change-log.md`
- Rename: `docs/rust/zh/building-blocks/agent/overview.mdx` → `docs/rust/zh/building-blocks/agent/overview.md`

**Interfaces:**
- Produces: `npm run docs:dev`, `npm run docs:build`, `npm run docs:preview`
- Produces: `sidebar: DefaultTheme.Sidebar` exported from `sidebar.mts`
- Produces: global Vue components named exactly `Note`, `Tip`, `Card`, `CardGroup`, `Badge`, `Accordion`, `AccordionGroup`
- Constraint: this task configures only the four prototype routes; full 50-page sidebar belongs to Task 6

- [ ] **Step 1: Record the baseline failure**

Run:

```bash
rtk find docs/rust/zh -type f -name '*.mdx' | wc -l
rtk find docs/rust/zh -type f -name '*.md' | wc -l
```

Expected: `50` `.mdx` pages and `0` `.md` pages.

- [ ] **Step 2: Create the pinned Node manifest**

Create `.nvmrc`:

```text
22
```

Create `package.json`:

```json
{
  "name": "agentscope-rust-docs",
  "private": true,
  "type": "module",
  "scripts": {
    "docs:dev": "vitepress dev docs/rust/zh",
    "docs:build": "vitepress build docs/rust/zh",
    "docs:preview": "vitepress preview docs/rust/zh --host 127.0.0.1",
    "docs:check": "node scripts/docs/check-docs.mjs",
    "docs:check-built": "node scripts/docs/check-built-site.mjs",
    "docs:check-examples": "node scripts/docs/check-examples.mjs",
    "test:docs": "node --test tests/docs/*.test.mjs",
    "test:docs:e2e": "playwright test"
  },
  "devDependencies": {
    "@axe-core/playwright": "4.10.2",
    "@playwright/test": "1.54.2",
    "vitepress": "1.6.4",
    "vue": "3.5.18"
  }
}
```

- [ ] **Step 3: Install and lock dependencies**

Run:

```bash
rtk npm install
```

Expected: `package-lock.json` created; VitePress resolves to `1.6.4`.

Verify:

```bash
rtk npm list vitepress vue @playwright/test @axe-core/playwright
```

Expected: all four packages are installed at the exact manifest versions.

- [ ] **Step 4: Rename only the four prototype pages**

Run:

```bash
rtk git mv docs/rust/zh/index.mdx docs/rust/zh/index.md
rtk git mv docs/rust/zh/others/faq.mdx docs/rust/zh/others/faq.md
rtk git mv docs/rust/zh/others/change-log.mdx docs/rust/zh/others/change-log.md
rtk git mv docs/rust/zh/building-blocks/agent/overview.mdx docs/rust/zh/building-blocks/agent/overview.md
```

Expected: 46 `.mdx` and 4 `.md` pages.

- [ ] **Step 5: Convert prototype Vue syntax and routes**

In the four prototype files:

- Replace `cols={1}` / `cols={2}` with `:cols="1"` / `:cols="2"`.
- Replace `/versions/0.1.0/zh/<path>` with `/<path>`.
- Remove links from `index.md` to `../STATUS-BLOCK.md` and `../mirror-map.md`, retaining equivalent plain-language status text.
- Replace any example/source relative link with the `https://github.com/NingNing0111/agentscope-rust/{tree|blob}/master/...` form from the spec.

Run:

```bash
rtk grep -Rn -E 'cols=\{[0-9]+\}|/versions/0\.1\.0/zh/|\]\(\.\./(STATUS-BLOCK|mirror-map)' \
  docs/rust/zh/index.md \
  docs/rust/zh/others/faq.md \
  docs/rust/zh/others/change-log.md \
  docs/rust/zh/building-blocks/agent/overview.md
```

Expected: no matches and exit code `1` from grep.

- [ ] **Step 6: Implement the minimal prototype sidebar and VitePress config**

Create `docs/rust/zh/.vitepress/sidebar.mts`:

```ts
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
```

Create `docs/rust/zh/.vitepress/config.mts`:

```ts
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
```

- [ ] **Step 7: Implement the seven global compatibility components**

Create `theme/index.ts` that imports `DefaultTheme`, registers the seven components by their exact names, and imports `custom.css`:

```ts
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
```

Use these exact public props:

```ts
// Card.vue
withDefaults(defineProps<{
  title: string
  icon?: string
  href?: string
  cta?: string
}>(), { icon: undefined, href: undefined, cta: undefined })

// CardGroup.vue
withDefaults(defineProps<{ cols?: number }>(), { cols: 1 })

// Badge.vue
withDefaults(defineProps<{
  color?: 'green' | 'blue' | 'amber' | 'gray'
  size?: 'sm' | 'md'
}>(), { color: 'gray', size: 'md' })

// Accordion.vue
defineProps<{ title: string }>()
```

`Accordion.vue` must use a native `<button type="button">`, a stable generated panel id, `aria-expanded`, `aria-controls`, and `v-show` for the panel. `Note.vue` and `Tip.vue` must include visible Chinese type labels so meaning is not color-only. `Card.vue` must use VitePress `VPLink` when `href` is present and a non-interactive `<div>` otherwise.

- [ ] **Step 8: Add minimal responsive theme CSS**

Implement CSS tokens using VitePress variables, including:

```css
.as-card-group {
  display: grid;
  grid-template-columns: repeat(var(--as-card-cols, 1), minmax(0, 1fr));
  gap: 1rem;
}

@media (max-width: 640px) {
  .as-card-group {
    grid-template-columns: 1fr;
  }
}
```

Add visible `:focus-visible` outlines for Card and Accordion buttons, and use `var(--vp-c-*)` colors for both themes.

- [ ] **Step 9: Build the four-page prototype**

Run:

```bash
rtk npm run docs:build
```

Expected: PASS; dist contains the four prototype routes.

Verify:

```bash
rtk find docs/rust/zh/.vitepress/dist -type f -name '*.html'
```

Expected: includes `index.html`, `others/faq.html`, `others/change-log.html`, and `building-blocks/agent/overview.html` or the corresponding clean-URL output layout.

- [ ] **Step 10: Manually inspect the prototype preview**

Run in background:

```bash
rtk npm run docs:preview
```

Open `/agentscope-rust/`, `/agentscope-rust/others/faq`, and `/agentscope-rust/building-blocks/agent/overview`. Verify CardGroup layout, Accordion operation, Badge inline layout, base path assets, dark theme, and mobile width.

Expected: no console errors and no missing component warnings.

- [ ] **Step 11: Stop if the prototype fails**

Do not continue to Task 2 until the four representative pages pass both build and preview. Fix the prototype in this task rather than weakening VitePress checks.

- [ ] **Step 12: Commit the prototype**

```bash
rtk git add .nvmrc package.json package-lock.json docs/rust/zh/.vitepress \
  docs/rust/zh/index.md docs/rust/zh/others/faq.md \
  docs/rust/zh/others/change-log.md docs/rust/zh/building-blocks/agent/overview.md
rtk git commit -m "feat(docs): prototype VitePress site"
```

---

### Task 2: 建立文档模型和 Markdown 扫描器的 TDD 基础

**Files:**
- Create: `scripts/docs/lib/docs-model.mjs`
- Create: `scripts/docs/lib/markdown-scan.mjs`
- Create: `tests/docs/docs-model.test.mjs`
- Create: `tests/docs/markdown-scan.test.mjs`

**Interfaces:**
- Produces: `parseMirrorMap(markdown: string): string[]`
- Produces: `listPageFiles(root: string, extension?: string): Promise<string[]>`
- Produces: `normalizePagePath(path: string): string`
- Produces: `flattenSidebar(items: unknown[]): string[]`
- Produces: `stripCode(markdown: string): string`
- Produces: `extractComponents(markdown: string): Map<string, Set<string>>`
- Produces: `extractLinks(markdown: string): Array<{ kind: 'markdown' | 'component', target: string }>`

- [ ] **Step 1: Write failing mirror-map and page normalization tests**

Create `tests/docs/docs-model.test.mjs` with fixtures that assert:

```js
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  flattenSidebar,
  normalizePagePath,
  parseMirrorMap
} from '../../scripts/docs/lib/docs-model.mjs'

test('parseMirrorMap returns Rust page paths only', () => {
  const input = [
    '| `index.mdx` | `index.md` | 已实现 | — | — | 首页 |',
    '| `building-blocks/agent/overview.mdx` | `building-blocks/agent/overview.md` | 已实现 | L2 | `agent` | Agent |'
  ].join('\n')
  assert.deepEqual(parseMirrorMap(input), [
    'index.md',
    'building-blocks/agent/overview.md'
  ])
})

test('normalizePagePath maps files and links to canonical routes', () => {
  assert.equal(normalizePagePath('index.md'), '/')
  assert.equal(normalizePagePath('others/faq.md'), '/others/faq')
  assert.equal(normalizePagePath('/others/faq/'), '/others/faq')
})

test('flattenSidebar recursively returns canonical links', () => {
  const sidebar = [{
    text: '开始',
    items: [{ text: '首页', link: '/' }, { text: 'FAQ', link: '/others/faq' }]
  }]
  assert.deepEqual(flattenSidebar(sidebar), ['/', '/others/faq'])
})
```

- [ ] **Step 2: Run the model tests and verify failure**

Run:

```bash
rtk node --test tests/docs/docs-model.test.mjs
```

Expected: FAIL because `scripts/docs/lib/docs-model.mjs` does not exist.

- [ ] **Step 3: Implement the minimal document model**

Implement the four exported functions. Requirements:

- `parseMirrorMap` matches only table rows whose third column is `已实现|部分支持|计划中` and returns the second backtick path.
- `listPageFiles` recursively walks with `node:fs/promises`, excludes `.vitepress`, sorts POSIX-style relative paths.
- `normalizePagePath` strips `.md`, strips trailing slash, maps `index` and nested `/index` correctly.
- `flattenSidebar` recursively visits `items`, returns normalized `link` values, and ignores external `http(s)` links.

- [ ] **Step 4: Run model tests and verify pass**

```bash
rtk node --test tests/docs/docs-model.test.mjs
```

Expected: all tests PASS.

- [ ] **Step 5: Write failing Markdown scanner tests**

Create `tests/docs/markdown-scan.test.mjs` with exact cases:

```js
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  extractComponents,
  extractLinks,
  stripCode
} from '../../scripts/docs/lib/markdown-scan.mjs'

test('stripCode removes fenced and inline code before component scanning', () => {
  const input = 'Use `<Vec>`\n```rust\nlet x: Vec<String>;\n<Msg>\n```\n<Note>real</Note>'
  assert.equal(stripCode(input).includes('<Vec>'), false)
  assert.equal(stripCode(input).includes('<Msg>'), false)
  assert.equal(stripCode(input).includes('<Note>'), true)
})

test('extractComponents returns component names and attributes', () => {
  const found = extractComponents('<Card title="A" href="/a"><Badge color="green" size="sm">New</Badge></Card>')
  assert.deepEqual([...found.get('Card')].sort(), ['href', 'title'])
  assert.deepEqual([...found.get('Badge')].sort(), ['color', 'size'])
})

test('extractLinks finds markdown and component links', () => {
  assert.deepEqual(extractLinks('[A](/a)\n<Card href="/b">B</Card>'), [
    { kind: 'markdown', target: '/a' },
    { kind: 'component', target: '/b' }
  ])
})
```

- [ ] **Step 6: Run scanner tests and verify failure**

```bash
rtk node --test tests/docs/markdown-scan.test.mjs
```

Expected: FAIL because the scanner module does not exist.

- [ ] **Step 7: Implement the minimal scanner**

Implement a deterministic scanner without a full MDX parser:

- remove triple-backtick/tilde fenced blocks while preserving line count;
- remove inline backtick spans;
- match only the seven allowlisted PascalCase component names;
- parse attribute names before `=` or boolean attributes;
- extract Markdown `](target)` links and `href="target"` from Card;
- strip URL fragments only when later resolving targets, not during extraction.

- [ ] **Step 8: Run all Node unit tests**

```bash
rtk npm run test:docs
```

Expected: all model and scanner tests PASS.

- [ ] **Step 9: Commit the scanner foundation**

```bash
rtk git add scripts/docs/lib tests/docs
rtk git commit -m "test(docs): add document model scanners"
```

---

### Task 3: 全量迁移 50 页并同步镜像维护文件

**Files:**
- Rename: remaining `docs/rust/zh/**/*.mdx` → matching `*.md`
- Modify: all `docs/rust/zh/**/*.md`
- Modify: `docs/rust/mirror-map.md`
- Modify: `docs/rust/README.md`
- Modify: `scripts/check-docs-mirror.sh`

**Interfaces:**
- Consumes: seven component names and Vue syntax from Task 1
- Consumes: `parseMirrorMap`/`listPageFiles` semantics from Task 2
- Produces: exactly 50 public `.md` pages and zero `.mdx` pages
- Produces: updated mirror map with Rust `.md` paths

- [ ] **Step 1: Add a temporary failing migration assertion**

Run:

```bash
rtk proxy bash -lc 'test "$(find docs/rust/zh -type f -name "*.mdx" | wc -l | tr -d " ")" = "0"'
```

Expected: FAIL because 46 `.mdx` pages remain.

- [ ] **Step 2: Rename the remaining pages without changing directory structure**

Use a null-delimited loop:

```bash
rtk proxy bash -lc 'while IFS= read -r -d "" file; do git mv "$file" "${file%.mdx}.md"; done < <(find docs/rust/zh -type f -name "*.mdx" -print0)'
```

Expected: `rtk git status --short` shows rename/delete+add pairs, not new parallel copies.

- [ ] **Step 3: Convert all JSX-style numeric props**

Replace all occurrences matching `cols={N}` with `:cols="N"` in public pages.

Verify:

```bash
if rtk grep -Rn -E 'cols=\{[0-9]+\}' docs/rust/zh --include='*.md'; then exit 1; fi
```

Expected: PASS with no matches.

- [ ] **Step 4: Convert old versioned internal routes**

Replace each `/versions/0.1.0/zh/<path>` with `/<path>`. Do not add `/agentscope-rust/` to content files.

Verify:

```bash
if rtk grep -Rn '/versions/0\.1\.0/zh/' docs/rust/zh docs/rust/README.md docs/rust/mirror-map.md; then exit 1; fi
```

Expected: initially FAIL until maintenance files are updated in Step 6, then PASS.

- [ ] **Step 5: Convert content-root-external links**

For each link from a public page to:

- `STATUS-BLOCK.md` / `mirror-map.md`: replace the link with reader-facing text and no hyperlink.
- directory under `examples/`: use `https://github.com/NingNing0111/agentscope-rust/tree/master/examples/<name>/`.
- concrete source file: use `https://github.com/NingNing0111/agentscope-rust/blob/master/<path>`.

Do not change legitimate page-to-page relative links.

Verify no relative target escapes `docs/rust/zh` by running a small one-off Node command using `extractLinks`; any resolved target outside the content root must be printed and fixed before continuing.

- [ ] **Step 6: Update mirror map and docs README policy**

In `docs/rust/mirror-map.md`:

- change all Rust page entries from `.mdx` to `.md`;
- replace the versioned-route statement with `文档内容使用站点根路由；部署前缀由 VitePress base 注入`;
- keep Python page paths unchanged as `.mdx`.

In `docs/rust/README.md`:

- update navigation paths to `.md`;
- document `npm run docs:dev`, `npm run docs:check`, `npm run docs:build`;
- state that only `docs/rust/zh/**/*.md` is published;
- replace the old versioned-route statement with the new base policy.

- [ ] **Step 7: Update the existing mirror checker for asymmetric extensions**

Modify `scripts/check-docs-mirror.sh`:

- normalize Python `.mdx` and Rust `.md` to extensionless relative paths before `comm`;
- rename `all_mdx` to `all_pages` and enumerate `*.md` under Rust docs;
- remove `.mdx` fallback when resolving Rust page links;
- keep status-block, planned-page, count-50 and relative-link checks.

Add this exact normalization concept:

```bash
py_files="$(cd "$PY_DOCS" && find . -type f -name '*.mdx' | sed -E 's|^\./||; s|\.mdx$||' | sort)"
rs_files="$(cd "$RS_DOCS" && find . -type f -name '*.md' | sed -E 's|^\./||; s|\.md$||' | sort)"
```

- [ ] **Step 8: Verify migration invariants**

Run:

```bash
rtk proxy bash -lc 'test "$(find docs/rust/zh -type f -name "*.mdx" | wc -l | tr -d " ")" = "0"'
rtk proxy bash -lc 'test "$(find docs/rust/zh -type f -name "*.md" | wc -l | tr -d " ")" = "50"'
rtk bash scripts/check-docs-mirror.sh
```

Expected: all commands PASS; mirror checker reports 50-page structural alignment and status checks.

- [ ] **Step 9: Build all discovered pages before full navigation**

Temporarily set sidebar to `undefined` or retain prototype sidebar while allowing route discovery, then run:

```bash
rtk npm run docs:build
```

Expected: PASS across all 50 Markdown pages; unknown component or Vue template failures must be fixed here.

- [ ] **Step 10: Commit the migration**

```bash
rtk git add docs/rust/zh docs/rust/mirror-map.md docs/rust/README.md scripts/check-docs-mirror.sh
rtk git commit -m "docs: migrate Rust guide to VitePress Markdown"
```

---

### Task 4: 实现 fail-closed 源文档检查器

**Files:**
- Create: `scripts/docs/check-docs.mjs`
- Modify: `tests/docs/docs-model.test.mjs`
- Modify: `tests/docs/markdown-scan.test.mjs`
- Create: `tests/docs/check-docs.test.mjs`
- Modify: `package.json`

**Interfaces:**
- Consumes: `parseMirrorMap`, `listPageFiles`, `normalizePagePath`, `flattenSidebar`
- Consumes: `extractComponents`, `extractLinks`
- Produces: `runChecks({ root, docsRoot, mirrorMapPath, sidebar }): Promise<string[]>`
- CLI contract: no output errors → exit `0`; any error → print one `ERROR:` line per issue and exit `1`

- [ ] **Step 1: Write failing aggregate checker tests**

Create fixture directories under a temporary directory in `tests/docs/check-docs.test.mjs`. Test at least:

1. valid two-page fixture returns `[]`;
2. mirror/file/sidebar mismatch reports all three normalized sets;
3. `.mdx` residue reports its path;
4. unknown `<Tabs>` component fails;
5. `Card foo="bar"` fails while allowed attributes pass;
6. component-looking tokens inside fenced Rust code do not fail;
7. `cols={2}` fails;
8. `/versions/0.1.0/zh/` fails;
9. `/agentscope-rust/` hardcoding fails;
10. relative link escaping docs root fails;
11. GitHub `blob/tree/master` path that does not exist locally fails.

- [ ] **Step 2: Run the tests and verify failure**

```bash
rtk node --test tests/docs/check-docs.test.mjs
```

Expected: FAIL because `check-docs.mjs` does not exist.

- [ ] **Step 3: Implement the component and attribute registry**

Use this exact registry in `check-docs.mjs`:

```js
const COMPONENTS = new Map([
  ['Note', new Set()],
  ['Tip', new Set()],
  ['Card', new Set(['title', 'icon', 'href', 'cta'])],
  ['CardGroup', new Set([':cols'])],
  ['Badge', new Set(['color', 'size'])],
  ['Accordion', new Set(['title'])],
  ['AccordionGroup', new Set()]
])
```

Any scanned component not in this map, or attribute outside the component set, appends a deterministic error.

- [ ] **Step 4: Implement the page-set invariant**

Load:

- `docs/rust/mirror-map.md` via `parseMirrorMap`;
- real `*.md` pages via `listPageFiles`;
- imported `sidebar` via `flattenSidebar`.

Normalize all three to routes and compare exact sorted sets. Error messages must state missing/extra values, not only counts.

- [ ] **Step 5: Implement route and repository-path policies**

For every public page after `stripCode`:

- reject `/versions/0.1.0/zh/`;
- reject `/agentscope-rust/` in content links;
- resolve relative page targets and reject targets outside `docs/rust/zh`;
- allow `http(s)` external links;
- for this repository's GitHub `blob/tree/master` URLs, map URL path back to repository root and require local existence;
- preserve anchors when reporting, but resolve paths without fragments.

- [ ] **Step 6: Implement the CLI wrapper**

Export `runChecks` for tests. Only execute `main()` when `import.meta.url === pathToFileURL(process.argv[1]).href`. Sort errors by path/message for stable CI output.

- [ ] **Step 7: Run unit tests**

```bash
rtk npm run test:docs
```

Expected: all checker/model/scanner tests PASS.

- [ ] **Step 8: Run the checker on the real 50-page corpus**

```bash
rtk npm run docs:check
```

Expected: PASS. Fix source documents or sidebar data; do not add broad ignore rules.

- [ ] **Step 9: Commit the source checker**

```bash
rtk git add scripts/docs/check-docs.mjs scripts/docs/lib tests/docs package.json package-lock.json
rtk git commit -m "feat(docs): enforce source documentation invariants"
```

---

### Task 5: 验证示例 package、命令和源码引用

**Files:**
- Create: `scripts/docs/check-examples.mjs`
- Create: `tests/docs/check-examples.test.mjs`
- Modify: `scripts/docs/lib/markdown-scan.mjs`
- Modify: public `docs/rust/zh/**/*.md` only where checks reveal invalid references

**Interfaces:**
- Produces: `extractCargoPackages(markdown: string): string[]`
- Produces: `extractRepositoryReferences(markdown: string): Array<{ path: string, type: 'tree' | 'blob' }>`
- Produces: CLI exit `0` when all referenced packages/paths exist; exit `1` with deterministic errors otherwise

- [ ] **Step 1: Write failing package extraction tests**

Test these exact inputs:

```js
assert.deepEqual(
  extractCargoPackages('`cargo run -p quickstart`\n```bash\ncargo check -p agent\n```'),
  ['agent', 'quickstart']
)
```

Also assert prose that merely mentions `cargo` without `-p` is ignored.

- [ ] **Step 2: Write failing repository URL tests**

Cover valid tree/blob URLs, case-insensitive owner matching, fragment removal, and rejection of paths outside `NingNing0111/agentscope-rust` mapping.

- [ ] **Step 3: Run tests and verify failure**

```bash
rtk node --test tests/docs/check-examples.test.mjs
```

Expected: FAIL because implementation is missing.

- [ ] **Step 4: Implement local package discovery**

Parse workspace member `Cargo.toml` files under `examples/*/Cargo.toml` and read the first `[package] name`. Do not shell out from unit-testable core functions.

Expected real package set:

```text
agent
chat
human-in-the-loop
mcp
memory
quickstart
rag
sandbox
skill
tool
workspace
```

- [ ] **Step 5: Implement reference checks**

For all 50 pages:

- every extracted `cargo ... -p NAME` must exist in workspace package set;
- every local repository GitHub URL must map to an existing path;
- report page, command/URL, and missing package/path.

- [ ] **Step 6: Run tests and real-corpus check**

```bash
rtk npm run test:docs
rtk npm run docs:check-examples
```

Expected: PASS. If a page claims a nonexistent package/path, correct the page rather than weakening the checker.

- [ ] **Step 7: Compile every public example package**

Run:

```bash
rtk proxy bash -lc 'for package in agent chat human-in-the-loop mcp memory quickstart rag sandbox skill tool workspace; do rtk cargo check -p "$package"; done'
```

Expected: all PASS without API credentials. This verifies compilation only; do not invoke real model calls.

- [ ] **Step 8: Commit example validation**

```bash
rtk git add scripts/docs/check-examples.mjs scripts/docs/lib/markdown-scan.mjs \
  tests/docs/check-examples.test.mjs docs/rust/zh
rtk git commit -m "test(docs): validate referenced examples and sources"
```

---

### Task 6: 完成产品首页、50 页导航和主题

**Files:**
- Modify: `docs/rust/zh/index.md`
- Modify: `docs/rust/zh/.vitepress/config.mts`
- Modify: `docs/rust/zh/.vitepress/sidebar.mts`
- Modify: `docs/rust/zh/.vitepress/theme/custom.css`
- Modify: compatibility components as needed from prototype feedback

**Interfaces:**
- Consumes: exactly 50 normalized routes from Task 4
- Produces: full `sidebar` whose normalized link set equals mirror map/file system
- Produces: nav labels `首页`, `快速开始`, `构建模块`, `部署与集成`, `FAQ`, `GitHub`

- [ ] **Step 1: Write a failing sidebar completeness assertion**

Run:

```bash
rtk npm run docs:check
```

Expected: FAIL because the prototype sidebar contains only four routes while mirror map/files contain 50.

- [ ] **Step 2: Replace prototype sidebar with the full explicit hierarchy**

Populate groups in this order:

1. 开始使用：`/`, `/quickstart`, `/release-notes`
2. Agent：five `building-blocks/agent/*` pages
3. Context：four `building-blocks/context/*` pages
4. Model：overview, llm, embedding, tts
5. Tool：overview, python-tool, mcp, skill, manage-tools
6. Workspace：overview, run-workspace, manage-resources, mcp-gateway
7. 构建模块：console, long-term-memory, message-and-event, middleware, permission-system pages, plan, rag
8. 部署与集成：all `deploy/**` pages
9. 其他：change-log, faq

Use page frontmatter titles for display labels where practical, but keep links explicit literals so review can inspect coverage.

- [ ] **Step 3: Finalize the top navigation**

Set:

```ts
nav: [
  { text: '首页', link: '/' },
  { text: '快速开始', link: '/quickstart' },
  { text: '构建模块', link: '/building-blocks/agent/overview' },
  { text: '部署与集成', link: '/deploy/agent-service' },
  { text: 'FAQ', link: '/others/faq' },
  { text: 'GitHub', link: 'https://github.com/NingNing0111/agentscope-rust' }
]
```

Keep `ignoreDeadLinks: false`, local search, `cleanUrls: true`, and `base: '/agentscope-rust/'`.

- [ ] **Step 4: Build the product homepage content**

Ensure `index.md` contains machine-testable text and links:

- H1 or hero title `AgentScope Rust`;
- positioning text containing `Rust` and `智能体开发框架`;
- `/quickstart` primary link;
- repository GitHub link;
- visible section `核心能力`;
- visible section `实现状态` containing `已实现`、`部分支持`、`计划中`;
- Cards for Agent, Tool/MCP, Memory/RAG, Workspace/Sandbox, Permission/HITL, Skill/SubAgent/任务规划.

Do not claim deploy/channel/hub features are implemented.

- [ ] **Step 5: Finalize responsive and theme styles**

Ensure:

- cards use one column below 640px;
- two-column groups do not overflow at tablet widths;
- Card/Accordion focus rings meet visible contrast;
- Note/Tip/Badge use text labels in addition to color;
- dark theme uses only VitePress theme tokens;
- no global body width or typography overrides break default theme.

- [ ] **Step 6: Run source checks and production build**

```bash
rtk npm run docs:check
rtk npm run docs:build
```

Expected: PASS; sidebar/file/mirror sets are equal.

- [ ] **Step 7: Commit navigation and homepage**

```bash
rtk git add docs/rust/zh/index.md docs/rust/zh/.vitepress
rtk git commit -m "feat(docs): add Chinese product homepage and navigation"
```

---

### Task 7: 实现生产构建产物检查器

**Files:**
- Create: `scripts/docs/check-built-site.mjs`
- Create: `tests/docs/check-built-site.test.mjs`
- Modify: `package.json`

**Interfaces:**
- Produces: `checkBuiltSite({ distRoot, base, expectedRoutes }): Promise<string[]>`
- Validates: route HTML presence, local `href/src` target existence, base prefix, forbidden leaked text/path

- [ ] **Step 1: Write failing fixture tests**

Create temporary dist fixtures for:

1. valid root and deep route with `/agentscope-rust/assets/app.js`;
2. missing route HTML;
3. asset URL without `/agentscope-rust/` base;
4. local link to missing route;
5. leaked strings `docs/python/`, `mirror-map.md`, `STATUS-BLOCK.md`, `docs/superpowers/`;
6. external `https://` URL ignored by local resolver.

- [ ] **Step 2: Run and verify failure**

```bash
rtk node --test tests/docs/check-built-site.test.mjs
```

Expected: FAIL because implementation is absent.

- [ ] **Step 3: Implement route-to-output resolution**

Support both VitePress clean-URL forms when checking expected routes:

- `/` → `dist/index.html`
- `/others/faq` → `dist/others/faq.html` or `dist/others/faq/index.html`

Fail only if neither valid output exists.

- [ ] **Step 4: Implement final HTML link and boundary scanning**

Without adding a DOM dependency, scan quoted `href` and `src` attributes in generated HTML. For local URLs:

- require `/agentscope-rust/` prefix;
- strip query/fragment;
- map asset and route paths to dist;
- require target existence.

Scan all dist text files for forbidden source-only identifiers.

- [ ] **Step 5: Run unit tests**

```bash
rtk npm run test:docs
```

Expected: all tests PASS.

- [ ] **Step 6: Check the real production build**

```bash
rtk npm run docs:build
rtk npm run docs:check-built
```

Expected: PASS over all 50 routes.

- [ ] **Step 7: Commit built-site checks**

```bash
rtk git add scripts/docs/check-built-site.mjs tests/docs/check-built-site.test.mjs package.json
rtk git commit -m "test(docs): validate VitePress build output"
```

---

### Task 8: 增加 Playwright smoke 与 axe 无障碍测试

**Files:**
- Create: `playwright.config.mts`
- Create: `tests/docs/site.spec.ts`
- Create: `tests/docs/accessibility.spec.ts`
- Modify: `package.json`

**Interfaces:**
- Consumes: `npm run docs:preview`
- Produces: `npm run test:docs:e2e`
- Base URL: `http://127.0.0.1:4173/agentscope-rust/`

- [ ] **Step 1: Install the Chromium browser binary**

```bash
rtk npx playwright install chromium
```

Expected: Chromium installs successfully.

- [ ] **Step 2: Create Playwright config**

Use:

```ts
import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests/docs',
  testMatch: /.*\.spec\.ts/,
  fullyParallel: false,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: 'http://127.0.0.1:4173/agentscope-rust/',
    trace: 'on-first-retry'
  },
  webServer: {
    command: 'npm run docs:preview -- --port 4173',
    url: 'http://127.0.0.1:4173/agentscope-rust/',
    reuseExistingServer: !process.env.CI
  },
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'] } },
    { name: 'mobile', use: { ...devices['Pixel 7'] } }
  ]
})
```

- [ ] **Step 3: Write failing homepage and deep-route smoke tests**

In `site.spec.ts`, assert:

- page title/heading includes `AgentScope Rust`;
- visible `快速开始` link resolves under `/agentscope-rust/quickstart`;
- GitHub link points to the repository;
- `实现状态`, `已实现`, `部分支持`, `计划中` are visible;
- direct navigation to `building-blocks/agent/overview` succeeds and survives reload.

Run before final selector fixes:

```bash
rtk npm run docs:build
rtk npx playwright test tests/docs/site.spec.ts --project=desktop
```

Expected: at least one assertion fails until homepage/component semantics are aligned.

- [ ] **Step 4: Add nav/sidebar/search/dark-mode tests**

Use role/text selectors rather than CSS implementation selectors. Test:

- desktop top nav opens expected route;
- mobile menu exposes `快速开始` and `FAQ`;
- sidebar link navigates to a known deep page;
- search dialog opens and finds `人机交互` or another stable known title;
- theme toggle changes root class or color-scheme state and persists after reload.

- [ ] **Step 5: Add Accordion and Card keyboard tests**

On `/others/faq`:

- focus first Accordion button;
- press Enter; assert `aria-expanded="true"` and panel visible;
- press Space; assert collapsed.

On homepage or Agent overview:

- focus a Card link with Tab or locator focus;
- press Enter;
- assert destination URL changes correctly under base.

- [ ] **Step 6: Write axe tests**

In `accessibility.spec.ts`, run `AxeBuilder` on:

- `/`;
- `/others/faq` with one accordion expanded;
- `/building-blocks/agent/overview`.

Filter results to `impact === 'serious' || impact === 'critical'` and assert empty. Run both desktop and mobile projects.

- [ ] **Step 7: Run all browser tests**

```bash
rtk npm run docs:build
rtk npm run test:docs:e2e
```

Expected: desktop and mobile smoke plus axe tests PASS.

- [ ] **Step 8: Commit browser validation**

```bash
rtk git add playwright.config.mts tests/docs/*.spec.ts package.json package-lock.json \
  docs/rust/zh/.vitepress/theme
rtk git commit -m "test(docs): add browser and accessibility checks"
```

---

### Task 9: 添加最小权限 GitHub Pages workflow

**Files:**
- Create: `.github/workflows/docs.yml`
- Modify: `package.json` only if a single CI aggregate script is added

**Interfaces:**
- Build artifact path: `docs/rust/zh/.vitepress/dist`
- Build permissions: `contents: read`
- Deploy permissions: `pages: write`, `id-token: write`
- Deploy condition: `workflow_dispatch` or `push` to `refs/heads/master`

- [ ] **Step 1: Write a failing static workflow assertion**

Before creating the workflow, run:

```bash
rtk proxy bash -lc 'test -f .github/workflows/docs.yml'
```

Expected: FAIL.

- [ ] **Step 2: Create workflow triggers and path filters**

Create `.github/workflows/docs.yml` with:

```yaml
name: Documentation

on:
  pull_request:
    branches: [master]
    paths:
      - 'docs/rust/zh/**'
      - 'docs/rust/README.md'
      - 'docs/rust/mirror-map.md'
      - 'scripts/docs/**'
      - 'scripts/check-docs-mirror.sh'
      - 'tests/docs/**'
      - 'playwright.config.mts'
      - 'package.json'
      - 'package-lock.json'
      - '.nvmrc'
      - '.github/workflows/docs.yml'
  push:
    branches: [master]
    paths:
      - 'docs/rust/zh/**'
      - 'docs/rust/README.md'
      - 'docs/rust/mirror-map.md'
      - 'scripts/docs/**'
      - 'scripts/check-docs-mirror.sh'
      - 'tests/docs/**'
      - 'playwright.config.mts'
      - 'package.json'
      - 'package-lock.json'
      - '.nvmrc'
      - '.github/workflows/docs.yml'
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: pages-${{ github.ref }}
  cancel-in-progress: true
```

- [ ] **Step 3: Add the read-only build job**

Use Ubuntu, checkout, Node 22 with npm cache, Pages configuration, `npm ci`, mirror check, source checks, example checks, build, built-site check, Chromium install, and e2e tests.

The sequence must be:

```yaml
- run: scripts/check-docs-mirror.sh --quiet
- run: npm run test:docs
- run: npm run docs:check
- run: npm run docs:check-examples
- run: npm run docs:build
- run: npm run docs:check-built
- run: npx playwright install --with-deps chromium
- run: npm run test:docs:e2e
```

Then upload exactly `docs/rust/zh/.vitepress/dist` using `actions/upload-pages-artifact`.

Do not grant build job Pages/OIDC write permissions.

- [ ] **Step 4: Add the gated deploy job**

Use:

```yaml
deploy:
  if: >-
    github.event_name == 'workflow_dispatch' ||
    (github.event_name == 'push' && github.ref == 'refs/heads/master')
  needs: build
  runs-on: ubuntu-latest
  permissions:
    pages: write
    id-token: write
  environment:
    name: github-pages
    url: ${{ steps.deployment.outputs.page_url }}
  steps:
    - name: Deploy to GitHub Pages
      id: deployment
      uses: actions/deploy-pages@v4
```

- [ ] **Step 5: Validate workflow structure locally**

Run a YAML parser available through Node, or add `yaml` as an exact dev dependency and execute a one-off parse. Then inspect permissions/conditions with grep:

```bash
rtk grep -n -E 'permissions:|contents: read|pages: write|id-token: write|github.event_name|refs/heads/master|upload-pages-artifact|deploy-pages' .github/workflows/docs.yml
```

Expected: `pages: write` and `id-token: write` appear only under deploy.

- [ ] **Step 6: Run the exact build-job commands locally**

```bash
rtk bash scripts/check-docs-mirror.sh --quiet
rtk npm ci
rtk npm run test:docs
rtk npm run docs:check
rtk npm run docs:check-examples
rtk npm run docs:build
rtk npm run docs:check-built
rtk npm run test:docs:e2e
```

Expected: all PASS.

- [ ] **Step 7: Commit the workflow**

```bash
rtk git add .github/workflows/docs.yml package.json package-lock.json
rtk git commit -m "ci(docs): publish VitePress site to GitHub Pages"
```

---

### Task 10: 更新仓库入口和管理员发布说明

**Files:**
- Modify: `README.md`
- Modify: `docs/rust/README.md`
- Create: `docs/rust/PUBLISHING.md`

**Interfaces:**
- Public site URL: `https://ningning0111.github.io/agentscope-rust/`
- Produces: explicit Pages enablement and first-deploy runbook

- [ ] **Step 1: Add a failing README link assertion**

Run:

```bash
rtk grep -n 'https://ningning0111.github.io/agentscope-rust/' README.md
```

Expected: FAIL/no match.

- [ ] **Step 2: Add the Chinese documentation site entry**

Under the root README introduction or a new `文档` section, add:

```markdown
## 文档

- [AgentScope Rust 中文文档](https://ningning0111.github.io/agentscope-rust/)
- [本地文档维护说明](docs/rust/README.md)
```

Keep existing pi-rust and skill links.

- [ ] **Step 3: Add the publishing runbook**

Create `docs/rust/PUBLISHING.md` with exact steps:

1. Repository `Settings → Pages → Build and deployment → Source → GitHub Actions`.
2. Merge workflow only after Pages source is enabled when possible.
3. If the first push happened earlier, trigger `Documentation` via `workflow_dispatch`.
4. Confirm build job succeeded and deploy job output `page_url`.
5. Verify root, quickstart, FAQ and deep Agent route.
6. If deploy fails because Pages was disabled, enable it and rerun; do not change permissions or push generated files to a branch.

This file is maintenance documentation and is not part of the public VitePress content root.

- [ ] **Step 4: Link publishing guidance from docs README**

Add a maintainer section linking `PUBLISHING.md` and listing the local verification commands.

- [ ] **Step 5: Verify links and build remain clean**

```bash
rtk grep -n 'https://ningning0111.github.io/agentscope-rust/' README.md
rtk npm run docs:check
rtk npm run docs:build
rtk npm run docs:check-built
```

Expected: all PASS.

- [ ] **Step 6: Commit repository documentation**

```bash
rtk git add README.md docs/rust/README.md docs/rust/PUBLISHING.md
rtk git commit -m "docs: add site and publishing guidance"
```

---

### Task 11: 执行最终全量验证与独立审查

**Files:**
- Modify: only files required by verified findings
- Test: entire documentation toolchain and referenced example packages

**Interfaces:**
- Consumes: all prior tasks
- Produces: evidence that all 15 spec acceptance criteria pass

- [ ] **Step 1: Verify clean migration counts**

```bash
rtk proxy bash -lc 'test "$(find docs/rust/zh -type f -name "*.mdx" | wc -l | tr -d " ")" = "0"'
rtk proxy bash -lc 'test "$(find docs/rust/zh -type f -name "*.md" | wc -l | tr -d " ")" = "50"'
```

Expected: PASS.

- [ ] **Step 2: Run formatting and diff checks**

```bash
rtk git diff --check
```

If project formatting scripts exist for Node/Vue after implementation, run them in check mode; do not introduce a formatter solely at this stage.

- [ ] **Step 3: Run all source and unit checks from a clean install**

```bash
rtk proxy rm -rf node_modules docs/rust/zh/.vitepress/dist
rtk npm ci
rtk bash scripts/check-docs-mirror.sh --quiet
rtk npm run test:docs
rtk npm run docs:check
rtk npm run docs:check-examples
```

Expected: all PASS.

- [ ] **Step 4: Build and inspect production output**

```bash
rtk npm run docs:build
rtk npm run docs:check-built
```

Expected: PASS; all 50 routes are present under the configured base.

- [ ] **Step 5: Run desktop/mobile browser and axe tests**

```bash
rtk npx playwright install chromium
rtk npm run test:docs:e2e
```

Expected: all projects PASS with no serious/critical axe findings.

- [ ] **Step 6: Compile every referenced example package**

```bash
rtk proxy bash -lc 'for package in agent chat human-in-the-loop mcp memory quickstart rag sandbox skill tool workspace; do rtk cargo check -p "$package"; done'
```

Expected: all PASS.

- [ ] **Step 7: Run existing Rust quality gates affected by documentation/example changes**

```bash
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace --doc
```

Expected: all PASS.

- [ ] **Step 8: Request independent code review**

Dispatch a reviewer that reads the approved spec and current diff, focusing on:

- VitePress `.md` migration completeness;
- component semantics and accessibility;
- route/base correctness;
- source and built-site checker false negatives;
- workflow permission boundaries and deploy condition;
- public-content exclusions;
- test coverage against the 15 acceptance criteria.

Any confirmed finding becomes a failing test or reproducible check before its fix.

- [ ] **Step 9: Apply findings with TDD and rerun relevant gates**

For each confirmed finding:

1. add/adjust the smallest failing unit, build, or Playwright test;
2. reproduce failure;
3. implement minimal fix;
4. rerun focused test;
5. rerun Steps 3–7 if the change affects shared configuration or content.

- [ ] **Step 10: Request independent verifier evidence**

The verifier must rerun the commands from Steps 1–7 and report exact pass/fail evidence. Do not use the authoring agent as the approval lane.

- [ ] **Step 11: Check GitHub Pages prerequisite and deployment**

Before merging or publishing, confirm the administrator has selected GitHub Actions as the Pages source. After `master` receives the workflow, verify either:

```bash
rtk gh run list --workflow docs.yml --limit 5
rtk gh pr checks
```

or the corresponding GitHub UI evidence. Confirm deploy output URL and fetch:

```bash
rtk curl https://ningning0111.github.io/agentscope-rust/
```

Expected: successful HTML response whose assets use `/agentscope-rust/`.

If Pages was not enabled before merge, enable it and use `workflow_dispatch`; do not claim deployment complete until this succeeds.

- [ ] **Step 12: Commit final verified fixes**

```bash
rtk git add -A
rtk git commit -m "fix(docs): address final site verification findings"
```

Skip this commit if review produced no changes. Record any external Pages-setting step separately in the final report because it is not represented by a repository diff.
