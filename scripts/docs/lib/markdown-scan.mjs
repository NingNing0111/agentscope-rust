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
