/**
 * Writes what each page costs to load: its gzipped weight with everything it references, its
 * requests, and the resources that block its first render. Also writes the gzipped size of each
 * image against the screen area it covers, the size of the script and the stylesheet, and whether
 * each style rule is used by some page.
 *
 * @module
 */

import * as fs from 'node:fs'

import * as Pages from './pages.js'

/** Bytes an image may weigh per pixel it covers on screen, and the least any image may weigh. */
const bytesPerPixel = 0.025
const smallestImage = 1000

const pages = Pages.load()
const lines: string[] = []

for (const page of pages) {
  const resources = Pages.resources(page)
  let weight = Bun.gzipSync(page.html).length
  for (const url of resources)
    weight += Bun.gzipSync(await Pages.serve(url)).length

  // A stylesheet in `<head>`, or a classic script without `defer` or `async`, stops the first
  // render until it loads.
  const blocking = page.elements.filter((element) => {
    if (!element.head) return false
    if (element.tag === 'link') return element.attributes.rel === 'stylesheet'
    if (element.tag !== 'script' || !element.attributes.src) return false
    return (
      element.attributes.type !== 'module' &&
      !('defer' in element.attributes) &&
      !('async' in element.attributes)
    )
  })

  lines.push(`page-weight ${page.name} ${weight}B`)
  lines.push(`requests ${page.name} ${1 + resources.length}`)
  lines.push(`render-blocking ${page.name} ${blocking.length}`)
}

// Each image's target comes from the largest area it covers on any page, so a hero image may
// weigh more than a logo.
const areas = new Map<string, number>()
for (const page of pages) {
  for (const image of page.elements.filter(
    (element) => element.tag === 'img',
  )) {
    const area =
      Number(image.attributes.width ?? 0) * Number(image.attributes.height ?? 0)
    areas.set(
      image.attributes.src!,
      Math.max(areas.get(image.attributes.src!) ?? 0, area),
    )
  }
}
for (const [src, area] of areas) {
  const target = Math.max(smallestImage, Math.round(area * bytesPerPixel))
  lines.push(
    `image-bytes ${src} ${Bun.gzipSync(await Pages.serve(src)).length}B target ${target}B`,
  )
}

lines.push(`script-size ${Bun.gzipSync(await Pages.serve('/app.js')).length}B`)

const css = fs.readFileSync('styles/site.css', 'utf8')
lines.push(`style-size ${Bun.gzipSync(css).length}B`)

// A rule is used when every tag, class, and ID in its selector appears on some page. Pseudo-classes
// such as `:root` and `:hover` match no element of their own, so they are left out.
const tags = new Set(
  pages.flatMap((page) => page.elements.map((element) => element.tag)),
)
const classes = new Set(
  pages.flatMap((page) =>
    page.elements.flatMap((element) =>
      (element.attributes.class ?? '').split(/\s+/),
    ),
  ),
)
const ids = new Set(
  pages.flatMap((page) =>
    page.elements.map((element) => element.attributes.id ?? ''),
  ),
)
for (const [, selectors] of css
  .replace(/\/\*[\s\S]*?\*\//g, '')
  .matchAll(/([^{}]+)\{[^}]*\}/g)) {
  for (const selector of selectors!
    .split(',')
    .map((selector) => selector.trim())) {
    const used = selector
      .replace(/::?[\w-]+(\([^)]*\))?/g, '')
      .split(/[\s>+~]+/)
      .flatMap((compound) => compound.match(/[.#]?[\w-]+/g) ?? [])
      .every((part) => {
        if (part.startsWith('.')) return classes.has(part.slice(1))
        if (part.startsWith('#')) return ids.has(part.slice(1))
        return tags.has(part)
      })
    lines.push(`used-styles ${Pages.quote(selector)} ${used ? 'pass' : 'fail'}`)
  }
}

Pages.write(lines)
