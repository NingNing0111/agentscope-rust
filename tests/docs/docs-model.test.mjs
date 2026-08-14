import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  flattenSidebar,
  listPageFiles,
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

test('parseMirrorMap ignores non-page table rows', () => {
  const input = [
    '| Python 页面 | Rust 页面 | 状态 |',
    '| --- | --- | --- |',
    '| `quickstart.mdx` | `quickstart.md` | 草稿 |',
    '| `release-notes.mdx` | `release-notes.md` | 部分支持 |'
  ].join('\n')
  assert.deepEqual(parseMirrorMap(input), ['release-notes.md'])
})

test('listPageFiles recursively lists sorted pages and excludes .vitepress', async () => {
  const root = await mkdtemp(join(tmpdir(), 'docs-model-'))
  try {
    await mkdir(join(root, 'guide'), { recursive: true })
    await mkdir(join(root, '.vitepress'), { recursive: true })
    await writeFile(join(root, 'z.md'), '')
    await writeFile(join(root, 'guide', 'a.md'), '')
    await writeFile(join(root, 'guide', 'ignored.mdx'), '')
    await writeFile(join(root, '.vitepress', 'hidden.md'), '')

    assert.deepEqual(await listPageFiles(root), ['guide/a.md', 'z.md'])
    assert.deepEqual(await listPageFiles(root, '.mdx'), ['guide/ignored.mdx'])
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})

test('normalizePagePath maps files and links to canonical routes', () => {
  assert.equal(normalizePagePath('index.md'), '/')
  assert.equal(normalizePagePath('others/faq.md'), '/others/faq')
  assert.equal(normalizePagePath('/others/faq/'), '/others/faq')
  assert.equal(normalizePagePath('building-blocks/index.md'), '/building-blocks')
})

test('flattenSidebar recursively returns canonical links', () => {
  const sidebar = [{
    text: '开始',
    items: [{ text: '首页', link: '/' }, { text: 'FAQ', link: '/others/faq' }]
  }]
  assert.deepEqual(flattenSidebar(sidebar), ['/', '/others/faq'])
})

test('flattenSidebar ignores external links case-insensitively', () => {
  const sidebar = [
    { text: 'GitHub', link: 'HtTpS://github.com/NingNing0111/agentscope-rust' },
    { text: 'HTTP', link: 'hTtP://example.com/docs' },
    { text: '开始', items: [{ text: '快速开始', link: '/quickstart.md' }] }
  ]
  assert.deepEqual(flattenSidebar(sidebar), ['/quickstart'])
})
