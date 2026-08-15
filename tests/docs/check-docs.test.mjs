import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runChecks } from '../../scripts/docs/check-docs.mjs'

async function createFixture({
  pages = {
    'index.md': '# 首页\n\n[指南](/guide)',
    'guide.md': '# 指南\n\n[首页](/)'
  },
  mirrorPages = ['index.md', 'guide.md'],
  sidebar = [
    { text: '首页', link: '/' },
    { text: '指南', link: '/guide' }
  ]
} = {}) {
  const root = await mkdtemp(join(tmpdir(), 'check-docs-'))
  const docsRoot = join(root, 'docs', 'rust', 'zh')
  const mirrorMapPath = join(root, 'docs', 'rust', 'mirror-map.md')
  await mkdir(docsRoot, { recursive: true })
  await writeFile(mirrorMapPath, mirrorPages.map((page) =>
    `| \`${page.replace(/\.md$/, '.mdx')}\` | \`${page}\` | 已实现 |`
  ).join('\n'))
  for (const [path, content] of Object.entries(pages)) {
    const target = join(docsRoot, path)
    await mkdir(join(target, '..'), { recursive: true })
    await writeFile(target, content)
  }
  return { root, docsRoot, mirrorMapPath, sidebar }
}

async function checkFixture(options) {
  const fixture = await createFixture(options)
  try {
    return await runChecks(fixture)
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
}

test('valid two-page fixture returns no errors', async () => {
  assert.deepEqual(await checkFixture(), [])
})

test('page-set mismatch reports every normalized mirror, file, and sidebar set', async () => {
  const errors = await checkFixture({
    pages: { 'index.md': '# 首页', 'extra.md': '# Extra' },
    mirrorPages: ['index.md', 'missing.md'],
    sidebar: [{ text: '首页', link: '/' }, { text: 'Other', link: '/other' }]
  })
  assert.ok(errors.some((error) => error.includes('mirror routes: /, /missing')))
  assert.ok(errors.some((error) => error.includes('file routes: /, /extra')))
  assert.ok(errors.some((error) => error.includes('sidebar routes: /, /other')))
})

test('reports residual mdx page path', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '# 首页',
    'guide.md': '# 指南',
    'legacy.mdx': '# Legacy'
  } })
  assert.ok(errors.some((error) => error.includes('legacy.mdx')))
})

test('rejects unknown components and unknown attributes but allows registered attributes', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '<Tabs>bad</Tabs>\n<Card title="Good" icon="x" href="/guide" cta="Go" foo="bar">Card</Card>',
    'guide.md': '<CardGroup :cols="2"><Badge color="green" size="sm">OK</Badge></CardGroup>'
  } })
  assert.ok(errors.some((error) => error.includes('unknown component <Tabs>')))
  assert.ok(errors.some((error) => error.includes('Card') && error.includes('foo')))
  assert.equal(errors.some((error) => /unknown attribute.*(?:title|icon|href|cta|:cols|color|size)/.test(error)), false)
})

test('ignores component-looking tokens inside fenced Rust code', async () => {
  assert.deepEqual(await checkFixture({ pages: {
    'index.md': '```rust\n<Tabs foo="bar">\nlet value: Vec<String>;\n```',
    'guide.md': '# Guide'
  } }), [])
})

test('rejects JSX cols syntax', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '<CardGroup cols={2}></CardGroup>',
    'guide.md': '# Guide'
  } })
  assert.ok(errors.some((error) => error.includes('cols={2}')))
})

test('rejects legacy and deployment-prefixed content routes', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '[Old](/versions/0.1.0/zh/guide)\n[Prefixed](/agentscope-rust/guide)',
    'guide.md': '# Guide'
  } })
  assert.ok(errors.some((error) => error.includes('/versions/0.1.0/zh/')))
  assert.ok(errors.some((error) => error.includes('/agentscope-rust/')))
})

test('rejects legacy and base-prefix routes written as plain prose', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '旧路由见 /versions/0.1.0/zh/guide，部署前缀见 /agentscope-rust/quickstart',
    'guide.md': '# Guide'
  } })
  assert.ok(errors.some((error) => error.includes('index.md') && error.includes('/versions/0.1.0/zh/')))
  assert.ok(errors.some((error) => error.includes('index.md') && error.includes('/agentscope-rust/quickstart')))
})

test('does not flag agentscope-rust inside GitHub repository URLs', async () => {
  const fixture = await createFixture({ pages: {
    'index.md': '见 [源码](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/demo)',
    'guide.md': '# Guide'
  } })
  try {
    await mkdir(join(fixture.root, 'examples', 'demo'), { recursive: true })
    assert.deepEqual(await runChecks(fixture), [])
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
})

test('rejects relative links that escape the docs root and preserves fragments in reports', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '[Private](../README.md#policy)',
    'guide.md': '# Guide'
  } })
  assert.ok(errors.some((error) => error.includes('../README.md#policy')))
})

test('rejects missing local paths in repository GitHub master URLs', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '[Missing](https://github.com/NingNing0111/agentscope-rust/blob/master/crates/not-here/src/lib.rs#L1)',
    'guide.md': '# Guide'
  } })
  assert.ok(errors.some((error) => error.includes('crates/not-here/src/lib.rs')))
})

test('accepts existing local paths in repository GitHub master URLs', async () => {
  const fixture = await createFixture({ pages: {
    'index.md': '[Source](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/demo#readme)',
    'guide.md': '# Guide'
  } })
  try {
    await mkdir(join(fixture.root, 'examples', 'demo'), { recursive: true })
    assert.deepEqual(await runChecks(fixture), [])
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
})
