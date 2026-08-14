import test from 'node:test'
import assert from 'node:assert/strict'
import {
  extractComponents,
  extractLinks,
  stripCode
} from '../../scripts/docs/lib/markdown-scan.mjs'

test('stripCode removes fenced and inline code before component scanning', () => {
  const input = 'Use `<Vec>`\n```rust\nlet x: Vec<String>;\n<Msg>\n```\n<Note>real</Note>'
  assert.equal(stripCode(input).includes('<Vec>'), false)
  assert.equal(stripCode(input).includes('<Msg>'), false)
  assert.equal(stripCode(input).includes('<Note>'), true)
})

test('stripCode preserves line count for backtick and tilde fences', () => {
  const input = 'before\n~~~md\n<Card href="/hidden">\n~~~\nafter'
  assert.equal(stripCode(input).split('\n').length, input.split('\n').length)
  assert.equal(stripCode(input).includes('/hidden'), false)
})

test('extractComponents returns component names and attributes', () => {
  const found = extractComponents('<Card title="A" href="/a"><Badge color="green" size="sm">New</Badge></Card>')
  assert.deepEqual([...found.get('Card')].sort(), ['href', 'title'])
  assert.deepEqual([...found.get('Badge')].sort(), ['color', 'size'])
})

test('extractComponents includes boolean attributes and ignores unknown components', () => {
  const found = extractComponents('<Accordion open title="A"><Unknown value="x" /></Accordion>')
  assert.deepEqual([...found.get('Accordion')].sort(), ['open', 'title'])
  assert.equal(found.has('Unknown'), false)
})

test('extractLinks finds markdown and component links', () => {
  assert.deepEqual(extractLinks('[A](/a)\n<Card href="/b">B</Card>'), [
    { kind: 'markdown', target: '/a' },
    { kind: 'component', target: '/b' }
  ])
})

test('extractLinks ignores links inside code and preserves fragments', () => {
  const input = '`[hidden](/code)`\n[FAQ](/others/faq#answer)\n```md\n<Card href="/hidden" />\n```'
  assert.deepEqual(extractLinks(input), [
    { kind: 'markdown', target: '/others/faq#answer' }
  ])
})
