/**
 * Reads the site for the metric scripts: every page under `pages/` as a list of elements, the
 * files that site URLs serve, and the metric file the scripts append to.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

/** An element of a page, with all the text inside it. */
export type Element = {
  attributes: Record<string, string>
  /** Whether the element is inside `<head>`. */
  head: boolean
  tag: string
  text: string
}

/** A page of the site. */
export type Page = {
  elements: readonly Element[]
  html: string
  /** The file name, such as `pricing.html`. */
  name: string
}

/** Reads every page under `pages/`, in name order. */
export function load(): readonly Page[] {
  return fs
    .readdirSync('pages')
    .filter((name) => name.endsWith('.html'))
    .sort()
    .map((name) => {
      const html = fs.readFileSync(path.join('pages', name), 'utf8')
      return { elements: elements(html), html, name }
    })
}

/** Returns the bytes a site URL serves: a file, or for `/app.js` the bundled `src/app.ts`. */
export async function serve(url: string): Promise<Uint8Array> {
  if (url === '/app.js') {
    const build = await Bun.build({ entrypoints: ['src/app.ts'], minify: true })
    return new Uint8Array(await build.outputs[0]!.arrayBuffer())
  }
  return fs.readFileSync(path.join('.', url))
}

/** Returns the elements on a page that reference other files: stylesheets, scripts, and images. */
export function resources(page: Page): readonly string[] {
  return page.elements.flatMap((element) => {
    if (element.tag === 'link' && element.attributes.rel === 'stylesheet')
      return [element.attributes.href!]
    if (element.tag === 'script' && element.attributes.src)
      return [element.attributes.src]
    if (element.tag === 'img' && element.attributes.src)
      return [element.attributes.src]
    return []
  })
}

/** Returns the metric line of a check, such as `image-alt "pricing.html /hero.svg" fail`. */
export function check(metric: string, item: string, passed: boolean): string {
  return `${metric} ${quote(item)} ${passed ? 'pass' : 'fail'}`
}

/** Quotes an item for a metric line, escaping `"` and `\`. */
export function quote(item: string): string {
  return `"${item.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`
}

/** Appends metric lines to the file at `$RETROS_METRICS`. */
export function write(lines: readonly string[]): void {
  fs.appendFileSync(process.env.RETROS_METRICS!, `${lines.join('\n')}\n`)
}

/** Parses HTML into its elements in document order. */
function elements(html: string): readonly Element[] {
  const found: Element[] = []
  const open: Element[] = []

  new HTMLRewriter()
    .on('*', {
      element(tag) {
        const element: Element = {
          attributes: Object.fromEntries(tag.attributes),
          head: open.some((parent) => parent.tag === 'head'),
          tag: tag.tagName,
          text: '',
        }
        found.push(element)

        if (!tag.canHaveContent || tag.selfClosing) return
        open.push(element)
        tag.onEndTag(() => {
          open.pop()
        })
      },
    })
    .onDocument({
      text(chunk) {
        for (const element of open) element.text += chunk.text
      },
    })
    .transform(html)

  return found
}
