/**
 * Writes whether the site follows web best practices: secure links, sized images, a doctype and a
 * character set on every page, and client code without console calls or deprecated APIs.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

import * as Pages from './pages.js'

/** APIs that browsers deprecate or that block the page while they run. */
const deprecated =
  /document\.write\(|addEventListener\(['"]unload['"]|new XMLHttpRequest\(/

const lines: string[] = []

for (const page of Pages.load()) {
  const charset = page.elements.some(
    (element) => element.tag === 'meta' && element.attributes.charset,
  )

  lines.push(
    Pages.check('doctype', page.name, /^<!doctype html>/i.test(page.html)),
  )
  lines.push(Pages.check('charset', page.name, charset))

  for (const element of page.elements) {
    const attributes = element.attributes
    const href = attributes.href ?? ''

    if (element.tag === 'a' && /^\w+:/.test(href)) {
      const secure = href.startsWith('https:')
      lines.push(Pages.check('secure-links', `${page.name} ${href}`, secure))
    }

    // An image without a width and height shifts the layout when it loads.
    if (element.tag === 'img') {
      const sized = 'width' in attributes && 'height' in attributes
      lines.push(
        Pages.check('image-size', `${page.name} ${attributes.src}`, sized),
      )
    }
  }
}

for (const file of new Bun.Glob('src/**/*.ts').scanSync()) {
  const source = fs.readFileSync(file, 'utf8')
  const name = path.basename(file)

  lines.push(
    Pages.check('console-calls', name, !/\bconsole\.\w+\(/.test(source)),
  )
  lines.push(Pages.check('deprecated-apis', name, !deprecated.test(source)))
}

Pages.write(lines)
