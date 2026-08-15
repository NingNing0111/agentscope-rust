import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { checkBuiltSite } from '../../scripts/docs/check-built-site.mjs'

const BASE = '/agentscope-rust/'

async function createFixture({ files = {}, routes = ['/'] } = {}) {
  const distRoot = await mkdtemp(join(tmpdir(), 'check-built-'))
  for (const [path, content] of Object.entries(files)) {
    const target = join(distRoot, path)
    await mkdir(join(target, '..'), { recursive: true })
    await writeFile(target, content)
  }
  return { distRoot, base: BASE, expectedRoutes: routes }
}

async function checkFixture(options) {
  const fixture = await createFixture(options)
  try {
    return await checkBuiltSite(fixture)
  } finally {
    await rm(fixture.distRoot, { recursive: true, force: true })
  }
}

test('valid root and deep route with based assets return no errors', async () => {
  const errors = await checkFixture({
    routes: ['/', '/others/faq'],
    files: {
      'index.html': '<html><head><script src="/agentscope-rust/assets/app.js"></script></head>' +
        '<body><a href="/agentscope-rust/others/faq">FAQ</a></body></html>',
      'others/faq.html': '<html><body><a href="/agentscope-rust/">首页</a></body></html>',
      'assets/app.js': 'console.log("ok")'
    }
  })
  assert.deepEqual(errors, [])
})

test('accepts the route index.html clean-URL output shape', async () => {
  const errors = await checkFixture({
    routes: ['/', '/others/faq'],
    files: {
      'index.html': '<a href="/agentscope-rust/others/faq">FAQ</a>',
      'others/faq/index.html': '<a href="/agentscope-rust/">首页</a>'
    }
  })
  assert.deepEqual(errors, [])
})

test('reports an expected route that has no output file', async () => {
  const errors = await checkFixture({
    routes: ['/', '/others/faq'],
    files: { 'index.html': '<html></html>' }
  })
  const routeErrors = errors.filter((error) => error.startsWith('missing route output'))
  assert.equal(routeErrors.length, 1)
  assert.ok(routeErrors[0].includes('/others/faq'))
})

test('reports asset URLs missing the base prefix', async () => {
  const errors = await checkFixture({
    files: {
      'index.html': '<script src="/assets/app.js"></script>',
      'assets/app.js': 'console.log("ok")'
    }
  })
  assert.ok(errors.some((error) => error.includes('missing base prefix') && error.includes('/assets/app.js')))
})

test('reports a local link whose route output is missing', async () => {
  const errors = await checkFixture({
    files: { 'index.html': '<a href="/agentscope-rust/quickstart">快速开始</a>' }
  })
  assert.ok(errors.some((error) => error.includes('missing target') && error.includes('/agentscope-rust/quickstart')))
})

test('reports forbidden source-only identifiers in dist text files', async () => {
  const errors = await checkFixture({
    routes: ['/', '/others/faq'],
    files: {
      'index.html': 'leak docs/python/ and mirror-map.md here',
      'others/faq.html': 'see STATUS-BLOCK.md and docs/superpowers/'
    }
  })
  for (const needle of ['docs/python/', 'mirror-map.md', 'STATUS-BLOCK.md', 'docs/superpowers/']) {
    assert.ok(
      errors.some((error) => error.includes(`forbidden text "${needle}"`)),
      `expected an error for ${needle}`
    )
  }
})

test('ignores external scheme URLs in the local resolver', async () => {
  const errors = await checkFixture({
    files: {
      'index.html': '<a href="https://github.com/NingNing0111/agentscope-rust">GitHub</a>' +
        '<img src="https://example.com/image.png" alt=""><a href="mailto:team@example.com">Mail</a>'
    }
  })
  assert.deepEqual(errors, [])
})

test('strips query and fragment when resolving local links', async () => {
  const errors = await checkFixture({
    routes: ['/', '/others/faq', '/quickstart'],
    files: {
      'index.html': '<a href="/agentscope-rust/others/faq#answer">FAQ</a>' +
        '<a href="/agentscope-rust/quickstart?from=home">开始</a>',
      'others/faq.html': '<a href="/agentscope-rust/">首页</a>',
      'quickstart.html': '<a href="/agentscope-rust/">首页</a>'
    }
  })
  assert.deepEqual(errors, [])
})

test('resolves relative in-content links against the page directory', async () => {
  const errors = await checkFixture({
    routes: ['/', '/building-blocks/agent/run-agent', '/building-blocks/model/llm'],
    files: {
      'index.html': '<a href="/agentscope-rust/">首页</a>',
      'building-blocks/agent/run-agent.html': '<a href="./../model/llm">模型</a><a href="./human-in-the-loop">人机交互</a>',
      'building-blocks/model/llm.html': '<a href="/agentscope-rust/">首页</a>',
      'building-blocks/agent/human-in-the-loop.html': '<a href="/agentscope-rust/">首页</a>'
    }
  })
  assert.deepEqual(errors, [])
})

test('reports a relative in-content link whose target is missing', async () => {
  const errors = await checkFixture({
    routes: ['/', '/building-blocks/agent/run-agent'],
    files: {
      'index.html': '<a href="/agentscope-rust/">首页</a>',
      'building-blocks/agent/run-agent.html': '<a href="./../model/llm">模型</a>'
    }
  })
  assert.ok(errors.some((error) => error.includes('missing target') && error.includes('./../model/llm')))
})

test('reports a missing dist directory instead of per-route noise', async () => {
  const distRoot = join(tmpdir(), `check-built-missing-${process.pid}-${Date.now()}`)
  try {
    const errors = await checkBuiltSite({ distRoot, base: BASE, expectedRoutes: ['/'] })
    assert.deepEqual(errors, [`dist directory not found: ${distRoot}`])
  } finally {
    await rm(distRoot, { recursive: true, force: true })
  }
})
