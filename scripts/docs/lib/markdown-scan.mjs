export function stripCode(markdown) {
  const lines = markdown.split('\n')
  let fence = null

  const withoutFences = lines.map((line) => {
    const marker = line.match(/^\s*(`{3,}|~{3,})/)
    if (!fence && marker) {
      fence = { character: marker[1][0], length: marker[1].length }
      return ''
    }
    if (fence) {
      const closing = line.match(/^\s*(`+|~+)/)
      if (
        closing &&
        closing[1][0] === fence.character &&
        closing[1].length >= fence.length
      ) {
        fence = null
      }
      return ''
    }
    return line
  }).join('\n')

  return withoutFences.replace(/(`+)[^\n]*?\1/g, '')
}

export function extractComponents(markdown) {
  const found = new Map()
  const content = stripCode(markdown)
  const tagPattern = /<([A-Z][A-Za-z0-9]*)(\s[^<>]*?)?\s*\/?>/g

  for (const match of content.matchAll(tagPattern)) {
    const [, name, source = ''] = match

    const attributes = found.get(name) ?? new Set()
    const attributePattern = /(?:^|\s)([:@]?[A-Za-z_][\w:.-]*)(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s]+))?/g
    for (const attribute of source.matchAll(attributePattern)) {
      attributes.add(attribute[1])
    }
    found.set(name, attributes)
  }

  return found
}

export function extractLinks(markdown) {
  const links = []
  const content = stripCode(markdown)

  for (const match of content.matchAll(/\[[^\]]*\]\(([^)\s]+)(?:\s+['"][^'"]*['"])?\)/g)) {
    links.push({ kind: 'markdown', target: match[1] })
  }

  for (const match of content.matchAll(/<Card\b[^<>]*\bhref\s*=\s*(['"])(.*?)\1[^<>]*>/g)) {
    links.push({ kind: 'component', target: match[2] })
  }

  return links
}

// A cargo command block: starts at `cargo`, spans the logical line including
// backslash line continuations; every `-p NAME` inside is a package reference.
const CARGO_COMMAND = /\bcargo\b[^\\\n]*(?:\\\r?\n[^\\\n]*)*/g

// Scans the raw markdown (fenced and inline code included) because cargo
// commands inside doc code blocks are meant to be validated.
export function extractCargoPackages(markdown) {
  const packages = new Set()
  for (const match of markdown.matchAll(CARGO_COMMAND)) {
    for (const pkg of match[0].matchAll(/\s-p\s+([A-Za-z0-9][A-Za-z0-9_-]*)/g)) {
      packages.add(pkg[1])
    }
  }
  return [...packages].sort()
}

const REPOSITORY_URL = /https:\/\/github\.com\/([^/?#\s]+)\/([^/?#\s]+)\/(blob|tree)\/([^/?#\s]+)\/([^?#\s)'"]+)/gi

// Extracts this repository's GitHub blob/tree URLs and maps them back to
// repository-root-relative paths (query/fragment stripped). The ref is kept
// so callers can enforce the master-only policy.
export function extractRepositoryReferences(markdown) {
  const references = []
  for (const match of markdown.matchAll(REPOSITORY_URL)) {
    const owner = match[1].toLowerCase()
    const repository = match[2].toLowerCase()
    if (owner !== 'ningning0111' || repository !== 'agentscope-rust') continue
    const path = decodeURIComponent(match[5].split(/[?#]/, 1)[0]).replace(/^\/+|\/+$/g, '')
    references.push({ path, type: match[3], ref: match[4] })
  }
  return references
}
