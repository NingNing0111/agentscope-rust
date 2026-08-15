import { access, readFile, readdir } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { listPageFiles } from './lib/docs-model.mjs'
import {
  extractCargoPackages,
  extractRepositoryReferences
} from './lib/markdown-scan.mjs'

async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

// Parses the first `[package] name` from every examples/*/Cargo.toml.
export async function discoverExamplePackages(examplesRoot) {
  const names = []
  const entries = await readdir(examplesRoot, { withFileTypes: true })
  for (const entry of entries) {
    if (!entry.isDirectory()) continue
    let manifest
    try {
      manifest = await readFile(join(examplesRoot, entry.name, 'Cargo.toml'), 'utf8')
    } catch {
      continue
    }
    const section = manifest.split('[package]', 2)[1] ?? ''
    const match = section.match(/^name\s*=\s*"([^"]+)"/m)
    if (match) names.push(match[1])
  }
  return names.sort()
}

function repositoryUrl({ path, type }) {
  return `https://github.com/NingNing0111/agentscope-rust/${type}/master/${path}`
}

function cargoCommand(content, name) {
  const pattern = new RegExp(`(?:^|\\s)-p\\s+${name}(?:\\s|$)`)
  for (const line of content.split('\n')) {
    if (/\bcargo\b/.test(line) && pattern.test(line)) return line.trim()
  }
  return `cargo ... -p ${name}`
}

export async function runChecks({ root, docsRoot, packageNames }) {
  const errors = []
  const packageSet = new Set(packageNames)
  const pageFiles = await listPageFiles(docsRoot)

  for (const pagePath of pageFiles) {
    const content = await readFile(resolve(docsRoot, pagePath), 'utf8')

    for (const name of extractCargoPackages(content)) {
      if (!packageSet.has(name)) {
        errors.push(`${pagePath}: unknown cargo package ${name} (command: ${cargoCommand(content, name)})`)
      }
    }

    for (const reference of extractRepositoryReferences(content)) {
      if (!await exists(resolve(root, reference.path))) {
        errors.push(`${pagePath}: repository path does not exist for ${repositoryUrl(reference)}`)
      }
    }
  }

  return errors.sort()
}

async function main() {
  const root = resolve(dirname(new URL(import.meta.url).pathname), '../..')
  const docsRoot = resolve(root, 'docs/rust/zh')
  const packageNames = await discoverExamplePackages(resolve(root, 'examples'))
  const errors = await runChecks({ root, docsRoot, packageNames })

  for (const error of errors) console.error(`ERROR: ${error}`)
  if (errors.length > 0) process.exitCode = 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main()
}
