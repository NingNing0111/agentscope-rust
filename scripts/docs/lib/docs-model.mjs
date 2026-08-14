import { readdir } from 'node:fs/promises'
import { relative, sep } from 'node:path'

const PAGE_STATUS = new Set(['已实现', '部分支持', '计划中'])

export function parseMirrorMap(markdown) {
  const pages = []

  for (const line of markdown.split(/\r?\n/)) {
    const columns = line.split('|').slice(1, -1).map((column) => column.trim())
    if (columns.length < 3 || !PAGE_STATUS.has(columns[2])) continue

    const match = columns[1].match(/^`([^`]+)`$/)
    if (match) pages.push(match[1])
  }

  return pages
}

export async function listPageFiles(root, extension = '.md') {
  const pages = []

  async function walk(directory) {
    const entries = await readdir(directory, { withFileTypes: true })
    for (const entry of entries) {
      if (entry.name === '.vitepress') continue

      const path = `${directory}/${entry.name}`
      if (entry.isDirectory()) {
        await walk(path)
      } else if (entry.isFile() && entry.name.endsWith(extension)) {
        pages.push(relative(root, path).split(sep).join('/'))
      }
    }
  }

  await walk(root)
  return pages.sort()
}

export function normalizePagePath(path) {
  let normalized = path.replace(/\\/g, '/').replace(/\.md$/, '')
  normalized = `/${normalized.replace(/^\/+|\/+$/g, '')}`
  normalized = normalized.replace(/\/index$/, '')
  return normalized || '/'
}

export function flattenSidebar(items) {
  const links = []

  for (const item of items) {
    if (!item || typeof item !== 'object') continue

    if (typeof item.link === 'string' && !/^https?:\/\//.test(item.link)) {
      links.push(normalizePagePath(item.link))
    }
    if (Array.isArray(item.items)) links.push(...flattenSidebar(item.items))
  }

  return links
}
