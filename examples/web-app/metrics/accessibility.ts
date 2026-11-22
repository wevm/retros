/**
 * Writes whether each page works with assistive technology: images with alternative text, named
 * buttons and links, labeled form fields, headings in order, and a page language. Also writes the
 * contrast ratio of each text color the stylesheet pairs with a background.
 *
 * @module
 */

import * as fs from 'node:fs'

import * as Pages from './pages.js'

/** Link text that says nothing about where the link goes. */
const vague = new Set(['click here', 'here', 'link', 'more', 'read more'])

/** Text colors and the backgrounds the stylesheet sets them on, by CSS custom property. */
const pairs = [
  { item: 'body text', text: '--ink', background: '--foam' },
  { item: 'navigation links', text: '--sea', background: '--foam' },
  { item: 'buttons', text: '--foam', background: '--sea' },
]

const lines: string[] = []

for (const page of Pages.load()) {
  const labels = new Set(
    page.elements
      .filter((element) => element.tag === 'label')
      .map((label) => label.attributes.for),
  )
  let buttons = 0

  for (const element of page.elements) {
    const attributes = element.attributes
    const name = (attributes['aria-label'] ?? element.text).trim()

    if (element.tag === 'img') {
      const item = `${page.name} ${attributes.src}`
      lines.push(Pages.check('image-alt', item, 'alt' in attributes))
    }

    if (element.tag === 'button') {
      const item = `${page.name} button ${++buttons}`
      lines.push(Pages.check('button-name', item, name !== ''))
    }

    if (element.tag === 'a') {
      const named = name !== '' && !vague.has(name.toLowerCase())
      lines.push(
        Pages.check('link-name', `${page.name} ${attributes.href}`, named),
      )
    }

    if (['input', 'select', 'textarea'].includes(element.tag)) {
      const labeled = labels.has(attributes.id) || 'aria-label' in attributes
      lines.push(
        Pages.check('field-label', `${page.name} #${attributes.id}`, labeled),
      )
    }
  }

  // Each heading may go at most one level deeper than the one before it.
  const levels = page.elements
    .filter((element) => /^h[1-6]$/.test(element.tag))
    .map((heading) => Number(heading.tag[1]))
  const ordered = levels.every(
    (level, index) => index === 0 || level <= levels[index - 1]! + 1,
  )
  lines.push(Pages.check('heading-order', page.name, ordered))

  const lang = page.elements.find((element) => element.tag === 'html')
    ?.attributes.lang
  lines.push(Pages.check('page-language', page.name, Boolean(lang)))
}

const css = fs.readFileSync('styles/site.css', 'utf8')
const colors = Object.fromEntries(
  [...css.matchAll(/(--[\w-]+):\s*(#[0-9a-f]{6})/gi)].map((match) => [
    match[1]!,
    match[2]!,
  ]),
)
for (const pair of pairs) {
  const ratio = contrast(colors[pair.text]!, colors[pair.background]!)
  lines.push(`contrast ${Pages.quote(pair.item)} ${ratio.toFixed(2)}`)
}

Pages.write(lines)

/** Returns the WCAG contrast ratio of two colors written as `#rrggbb`. */
function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (light! + 0.05) / (dark! + 0.05)
}

/** Returns the WCAG relative luminance of a color written as `#rrggbb`. */
function luminance(color: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => {
    const channel = Number.parseInt(color.slice(at, at + 2), 16) / 255
    return channel <= 0.039_28
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!
}
