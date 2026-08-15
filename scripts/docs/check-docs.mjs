import { access, readFile } from 'node:fs/promises'
import { dirname, isAbsolute, relative, resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'
import {
  flattenSidebar,
  listPageFiles,
  normalizePagePath,
  parseMirrorMap
} from './lib/docs-model.mjs'
import { extractComponents, extractLinks, stripCode } from './lib/markdown-scan.mjs'

const COMPONENTS = new Map([
  ['Note', new Set()],
  ['Tip', new Set()],
  ['Card', new Set(['title', 'icon', 'href', 'cta'])],
  ['CardGroup', new Set([':cols'])],
  ['Badge', new Set(['color', 'size'])],
  ['Accordion', new Set(['title'])],
  ['AccordionGroup', new Set()]
])

const REPOSITORY_URL = /^https:\/\/github\.com\/NingNing0111\/agentscope-rust\/(?:blob|tree)\/master\/(.+)$/i

function sortedRoutes(paths) {
  return [...new Set(paths.map(normalizePagePath))].sort()
}

function formatRoutes(name, routes) {
  return `${name} routes: ${routes.join(', ') || '(empty)'}`
}

function isOutside(root, target) {
  const path = relative(root, target)
  return path === '..' || path.startsWith(`..${sep}`) || isAbsolute(path)
}

async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

function addPageSetErrors(errors, mirrorRoutes, fileRoutes, sidebarRoutes) {
  const same = mirrorRoutes.length === fileRoutes.length &&
    mirrorRoutes.length === sidebarRoutes.length &&
    mirrorRoutes.every((route, index) => route === fileRoutes[index] && route === sidebarRoutes[index])
  if (same) return

  errors.push(`page-set mismatch: ${formatRoutes('mirror', mirrorRoutes)}`)
  errors.push(`page-set mismatch: ${formatRoutes('file', fileRoutes)}`)
  errors.push(`page-set mismatch: ${formatRoutes('sidebar', sidebarRoutes)}`)
}

async function checkPage({ content, docsRoot, pagePath, root }, errors) {
  const scanned = stripCode(content)
  const components = extractComponents(content)

  for (const [component, attributes] of components) {
    const allowed = COMPONENTS.get(component)
    if (!allowed) {
      errors.push(`${pagePath}: unknown component <${component}>`)
      continue
    }
    for (const attribute of [...attributes].sort()) {
      if (!allowed.has(attribute)) {
        errors.push(`${pagePath}: unknown attribute ${attribute} on <${component}>`)
      }
    }
  }

  for (const match of scanned.matchAll(/cols=\{\d+\}/g)) {
    errors.push(`${pagePath}: forbidden JSX attribute ${match[0]}`)
  }

  for (const link of extractLinks(content)) {
    const target = link.target
    if (target.includes('/versions/0.1.0/zh/')) {
      errors.push(`${pagePath}: forbidden legacy route ${target}`)
    }
    if (target.startsWith('/agentscope-rust/')) {
      errors.push(`${pagePath}: forbidden deployment prefix ${target}`)
    }

    const repositoryUrl = target.match(REPOSITORY_URL)
    if (repositoryUrl) {
      const repositoryPath = decodeURIComponent(repositoryUrl[1].split(/[?#]/, 1)[0])
      if (!await exists(resolve(root, repositoryPath))) {
        errors.push(`${pagePath}: repository path does not exist for ${target} (${repositoryPath})`)
      }
      continue
    }

    if (/^[a-z][a-z\d+.-]*:/i.test(target) || target.startsWith('//') || target.startsWith('#') || target.startsWith('/')) {
      continue
    }

    const pathOnly = target.split(/[?#]/, 1)[0]
    if (!pathOnly) continue
    const resolved = resolve(dirname(resolve(docsRoot, pagePath)), decodeURIComponent(pathOnly))
    if (isOutside(docsRoot, resolved)) {
      errors.push(`${pagePath}: relative link escapes docs root: ${target}`)
    }
  }
}

export async function runChecks({ root, docsRoot, mirrorMapPath, sidebar }) {
  const errors = []
  const mirrorMarkdown = await readFile(mirrorMapPath, 'utf8')
  const pageFiles = await listPageFiles(docsRoot)
  const mdxFiles = await listPageFiles(docsRoot, '.mdx')

  for (const path of mdxFiles) errors.push(`${path}: residual .mdx page`)

  addPageSetErrors(
    errors,
    sortedRoutes(parseMirrorMap(mirrorMarkdown)),
    sortedRoutes(pageFiles),
    [...new Set(flattenSidebar(sidebar))].sort()
  )

  for (const pagePath of pageFiles) {
    const content = await readFile(resolve(docsRoot, pagePath), 'utf8')
    await checkPage({ content, docsRoot, pagePath, root }, errors)
  }

  return errors.sort()
}

async function main() {
  const root = resolve(dirname(new URL(import.meta.url).pathname), '../..')
  const docsRoot = resolve(root, 'docs/rust/zh')
  const mirrorMapPath = resolve(root, 'docs/rust/mirror-map.md')
  const { sidebar } = await import(pathToFileURL(resolve(docsRoot, '.vitepress/sidebar.mts')).href)
  const errors = await runChecks({ root, docsRoot, mirrorMapPath, sidebar })

  for (const error of errors) console.error(`ERROR: ${error}`)
  if (errors.length > 0) process.exitCode = 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main()
}
