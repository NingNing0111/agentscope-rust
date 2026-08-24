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

// Parses `[package] name` values from Cargo.toml files directly below a
// directory. Kept exported for unit tests and for the historical examples-only
// discovery contract.
export async function discoverExamplePackages(examplesRoot) {
  const names = []
  let entries
  try {
    entries = await readdir(examplesRoot, { withFileTypes: true })
  } catch {
    return []
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) continue
    const manifestPath = join(examplesRoot, entry.name, 'Cargo.toml')
    let manifest
    try {
      manifest = await readFile(manifestPath, 'utf8')
    } catch {
      continue
    }
    const section = manifest.split('[package]', 2)[1] ?? ''
    const match = section.match(/^name\s*=\s*"([^"]+)"/m)
    if (match) names.push(match[1])
  }

  return [...new Set(names)].sort()
}

// Parses the first `[package] name` from Cargo.toml files in the root package,
// crates/*, and examples/*. Docs may reference both example packages (for
// runnable demos) and library crates (for feature-gated cargo check/test
// commands).
export async function discoverCargoPackages(root) {
  const names = []

  async function addManifest(manifestPath) {
    let manifest
    try {
      manifest = await readFile(manifestPath, 'utf8')
    } catch {
      return
    }
    const section = manifest.split('[package]', 2)[1] ?? ''
    const match = section.match(/^name\s*=\s*"([^"]+)"/m)
    if (match) names.push(match[1])
  }

  await addManifest(join(root, 'Cargo.toml'))

  for (const directory of ['crates', 'examples']) {
    const directoryPath = join(root, directory)
    let entries
    try {
      entries = await readdir(directoryPath, { withFileTypes: true })
    } catch {
      continue
    }
    for (const entry of entries) {
      if (entry.isDirectory()) await addManifest(join(directoryPath, entry.name, 'Cargo.toml'))
    }
  }

  return [...new Set(names)].sort()
}

function repositoryUrl({ path, type, ref }) {
  return `https://github.com/NingNing0111/agentscope-rust/${type}/${ref}/${path}`
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
      if (reference.ref !== 'master') {
        errors.push(
          `${pagePath}: repository URL uses non-master ref ${reference.ref} for ${repositoryUrl(reference)}`
        )
      }
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
  const packageNames = await discoverCargoPackages(root)
  const errors = await runChecks({ root, docsRoot, packageNames })

  for (const error of errors) console.error(`ERROR: ${error}`)
  if (errors.length > 0) process.exitCode = 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main()
}
