import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  extractCargoPackages,
  extractRepositoryReferences
} from '../../scripts/docs/lib/markdown-scan.mjs'
import {
  discoverExamplePackages,
  runChecks
} from '../../scripts/docs/check-examples.mjs'

const PACKAGES = [
  'agent',
  'chat',
  'human-in-the-loop',
  'mcp',
  'memory',
  'quickstart',
  'rag',
  'sandbox',
  'skill',
  'tool',
  'workspace'
]

test('extractCargoPackages returns the exact plan case', () => {
  assert.deepEqual(
    extractCargoPackages('`cargo run -p quickstart`\n```bash\ncargo check -p agent\n```'),
    ['agent', 'quickstart']
  )
})

test('extractCargoPackages ignores prose that mentions cargo without -p', () => {
  assert.deepEqual(
    extractCargoPackages('运行 cargo 前请先安装 Rust 工具链，然后使用 cargo run 启动示例。'),
    []
  )
})

test('extractCargoPackages deduplicates repeated package references', () => {
  assert.deepEqual(
    extractCargoPackages('`cargo run -p agent`\n`cargo check -p agent`\n```bash\ncargo build -p quickstart\n```'),
    ['agent', 'quickstart']
  )
})

test('extractRepositoryReferences extracts tree and blob master URLs', () => {
  const markdown = [
    '[Agent](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent)',
    '[Main](https://github.com/NingNing0111/agentscope-rust/blob/master/examples/chat/src/main.rs)'
  ].join('\n')
  assert.deepEqual(extractRepositoryReferences(markdown), [
    { path: 'examples/agent', type: 'tree' },
    { path: 'examples/chat/src/main.rs', type: 'blob' }
  ])
})

test('extractRepositoryReferences matches owners and repositories case-insensitively', () => {
  assert.deepEqual(
    extractRepositoryReferences('[X](https://github.com/ningning0111/AGENTSCOPE-RUST/tree/master/examples/agent/)'),
    [{ path: 'examples/agent', type: 'tree' }]
  )
})

test('extractRepositoryReferences strips fragments and queries from paths', () => {
  assert.deepEqual(
    extractRepositoryReferences('[X](https://github.com/NingNing0111/agentscope-rust/blob/master/examples/chat/src/main.rs#L10)'),
    [{ path: 'examples/chat/src/main.rs', type: 'blob' }]
  )
})

test('extractRepositoryReferences ignores other repositories and hosts', () => {
  const markdown = [
    '[A](https://github.com/other/agentscope-rust/tree/master/examples/agent)',
    '[B](https://gitlab.com/NingNing0111/agentscope-rust/blob/master/docs/rust/zh/index.md)',
    '[C](https://github.com/NingNing0111/agentscope-rust)'
  ].join('\n')
  assert.deepEqual(extractRepositoryReferences(markdown), [])
})

async function createFixture({ pages, packageNames = PACKAGES } = {}) {
  const root = await mkdtemp(join(tmpdir(), 'check-examples-'))
  const docsRoot = join(root, 'docs', 'rust', 'zh')
  await mkdir(docsRoot, { recursive: true })
  for (const [path, content] of Object.entries(pages)) {
    const target = join(docsRoot, path)
    await mkdir(join(target, '..'), { recursive: true })
    await writeFile(target, content)
  }
  return { root, docsRoot, packageNames }
}

async function checkFixture(options) {
  const fixture = await createFixture(options)
  try {
    return await runChecks(fixture)
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
}

test('unknown cargo package is reported with the command', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '```bash\ncargo run -p nope -- --flag\n```'
  } })
  assert.ok(errors.some((error) =>
    error.includes('index.md') &&
    error.includes('unknown cargo package nope') &&
    error.includes('command: cargo run -p nope -- --flag')
  ))
})

test('repository path that does not exist locally is reported', async () => {
  const errors = await checkFixture({ pages: {
    'index.md': '[Missing](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/missing/)'
  } })
  assert.ok(errors.some((error) =>
    error.includes('repository path does not exist for') &&
    error.includes('examples/missing')
  ))
})

test('valid packages and existing repository paths pass', async () => {
  const fixture = await createFixture({ pages: {
    'index.md': [
      '```bash',
      'cargo run -p agent',
      '```',
      '[Chat](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/chat/)',
      '[Main](https://github.com/NingNing0111/agentscope-rust/blob/master/examples/chat/src/main.rs)'
    ].join('\n')
  } })
  try {
    await mkdir(join(fixture.root, 'examples', 'chat', 'src'), { recursive: true })
    await writeFile(join(fixture.root, 'examples', 'chat', 'src', 'main.rs'), 'fn main() {}')
    assert.deepEqual(await runChecks(fixture), [])
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
})

test('discoverExamplePackages reads [package] names from example manifests', async () => {
  const root = await mkdtemp(join(tmpdir(), 'check-examples-'))
  try {
    await mkdir(join(root, 'examples', 'demo', 'src'), { recursive: true })
    await mkdir(join(root, 'examples', 'other'), { recursive: true })
    await writeFile(join(root, 'examples', 'demo', 'Cargo.toml'), '[package]\nname = "demo"\nversion.workspace = true\n')
    await writeFile(join(root, 'examples', 'other', 'Cargo.toml'), '[package]\nname = "other"\n')
    await writeFile(join(root, 'examples', 'README.md'), 'not a manifest')
    assert.deepEqual(await discoverExamplePackages(join(root, 'examples')), ['demo', 'other'])
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})
