const COMPONENTS = new Set([
  'Note',
  'Tip',
  'Card',
  'CardGroup',
  'Badge',
  'Accordion',
  'AccordionGroup'
])

export function stripCode(markdown) {
  const lines = markdown.split('\n')
  let fence = null

  const withoutFences = lines.map((line) => {
    const marker = line.match(/^\s*(`{3,}|~{3,})/)
    if (!fence && marker) {
      fence = marker[1][0]
      return ''
    }
    if (fence) {
      if (new RegExp(`^\\s*${fence}{3,}`).test(line)) fence = null
      return ''
    }
    return line
  }).join('\n')

  return withoutFences.replace(/`[^`\n]*`/g, '')
}

export function extractComponents(markdown) {
  const found = new Map()
  const content = stripCode(markdown)
  const tagPattern = /<([A-Z][A-Za-z0-9]*)(\s[^<>]*?)?\s*\/?>/g

  for (const match of content.matchAll(tagPattern)) {
    const [, name, source = ''] = match
    if (!COMPONENTS.has(name)) continue

    const attributes = found.get(name) ?? new Set()
    const attributePattern = /(?:^|\s)([:@]?[A-Za-z_][\w:.-]*)(?=\s*=|\s|$)/g
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
