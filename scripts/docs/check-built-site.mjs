import { access, readdir, readFile } from 'node:fs/promises'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'
import { flattenSidebar } from './lib/docs-model.mjs'

const FORBIDDEN_TEXT = ['docs/python/', 'mirror-map.md', 'STATUS-BLOCK.md', 'docs/superpowers/']
const URL_ATTRIBUTE = /(?<![\w-])(?:href|src)\s*=\s*(["'])(.*?)\1/g

async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

function normalizeBase(base) {
  if (!base.startsWith('/')) base = `/${base}`
  if (!base.endsWith('/')) base = `${base}/`
  return base
}

function stripQueryFragment(url) {
  return url.split(/[?#]/, 1)[0]
}

function isOutside(root, target) {
  const path = relative(root, target)
  return path === '..' || path.startsWith(`..${sep}`) || isAbsolute(path)
}

function outputCandidates(distRoot, pathOnly) {
  if (!pathOnly) return [join(distRoot, 'index.html')]
  if (pathOnly.endsWith('/')) return [join(distRoot, pathOnly, 'index.html')]
  if (/\.[a-z0-9]+$/i.test(pathOnly)) return [join(distRoot, pathOnly)]
  return [join(distRoot, `${pathOnly}.html`), join(distRoot, pathOnly, 'index.html')]
}

function formatOutputs(candidates, distRoot) {
  return candidates.map((candidate) => relative(distRoot, candidate).split(sep).join('/')).join(' or ')
}

async function anyExists(candidates) {
  for (const candidate of candidates) {
    if (await exists(candidate)) return true
  }
  return false
}

async function walkFiles(directory) {
  const files = []

  async function walk(current) {
    const entries = await readdir(current, { withFileTypes: true })
    for (const entry of entries) {
      const path = join(current, entry.name)
      if (entry.isDirectory()) {
        await walk(path)
      } else if (entry.isFile()) {
        files.push(path)
      }
    }
  }

  await walk(directory)
  return files
}

async function checkExpectedRoutes({ distRoot, expectedRoutes }, errors) {
  for (const route of [...new Set(expectedRoutes)]) {
    const pathOnly = stripQueryFragment(route).replace(/^\/+/, '')
    const candidates = outputCandidates(distRoot, pathOnly)
    if (!await anyExists(candidates)) {
      errors.push(`missing route output: ${route} (expected ${formatOutputs(candidates, distRoot)})`)
    }
  }
}

async function checkHtmlLinks(filePath, fileLabel, base, distRoot, errors) {
  const html = await readFile(filePath, 'utf8')

  for (const match of html.matchAll(URL_ATTRIBUTE)) {
    const url = match[2]
    if (!url || /^[a-z][a-z\d+.-]*:/i.test(url) || url.startsWith('//') || url.startsWith('#')) continue

    const pathOnly = stripQueryFragment(url)

    if (pathOnly.startsWith('/')) {
      if (!pathOnly.startsWith(base)) {
        errors.push(`${fileLabel}: missing base prefix for ${url} (expected prefix ${base})`)
        continue
      }

      const candidates = outputCandidates(distRoot, pathOnly.slice(base.length))
      if (!await anyExists(candidates)) {
        errors.push(`${fileLabel}: missing target for ${url} (expected ${formatOutputs(candidates, distRoot)})`)
      }
      continue
    }

    const resolved = resolve(dirname(filePath), pathOnly)
    if (isOutside(distRoot, resolved)) {
      errors.push(`${fileLabel}: missing target for ${url} (resolves outside dist)`)
      continue
    }

    const candidates = outputCandidates(distRoot, relative(distRoot, resolved).split(sep).join('/'))
    if (!await anyExists(candidates)) {
      errors.push(`${fileLabel}: missing target for ${url} (expected ${formatOutputs(candidates, distRoot)})`)
    }
  }
}

async function checkForbiddenText(filePath, fileLabel, errors) {
  const content = await readFile(filePath, 'utf8')
  if (content.includes('\u0000')) return

  for (const needle of FORBIDDEN_TEXT) {
    if (content.includes(needle)) {
      errors.push(`${fileLabel}: forbidden text ${JSON.stringify(needle)}`)
    }
  }
}

export async function checkBuiltSite({ distRoot, base, expectedRoutes }) {
  const errors = []
  const normalizedBase = normalizeBase(base)

  if (!await exists(distRoot)) {
    errors.push(`dist directory not found: ${distRoot}`)
    return errors
  }

  await checkExpectedRoutes({ distRoot, expectedRoutes }, errors)

  const files = await walkFiles(distRoot)
  for (const file of files.filter((path) => path.endsWith('.html'))) {
    await checkHtmlLinks(file, relative(distRoot, file).split(sep).join('/'), normalizedBase, distRoot, errors)
  }
  for (const file of files) {
    await checkForbiddenText(file, relative(distRoot, file).split(sep).join('/'), errors)
  }

  return errors.sort()
}

async function main() {
  const root = resolve(dirname(new URL(import.meta.url).pathname), '../..')
  const docsRoot = resolve(root, 'docs/rust/zh')
  const distRoot = resolve(docsRoot, '.vitepress/dist')
  const base = '/agentscope-rust/'
  const { sidebar } = await import(pathToFileURL(resolve(docsRoot, '.vitepress/sidebar.mts')).href)
  const expectedRoutes = [...new Set(flattenSidebar(sidebar))].sort()
  const errors = await checkBuiltSite({ distRoot, base, expectedRoutes })

  for (const error of errors) console.error(`ERROR: ${error}`)
  if (errors.length > 0) process.exitCode = 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main()
}
