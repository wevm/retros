/**
 * Writes whether each page is ready for search engines: the length of its title, and whether it
 * has a description, a canonical link, a viewport, one top-level heading, and links crawlers can
 * follow.
 *
 * @module
 */

import * as Pages from './pages.js'

const lines: string[] = []

for (const page of Pages.load()) {
  const tags = (tag: string) =>
    page.elements.filter((element) => element.tag === tag)
  const meta = (name: string) =>
    tags('meta').find((element) => element.attributes.name === name)

  // Search results cut titles at about 60 characters.
  const title = tags('title')[0]?.text.trim() ?? ''
  const canonical = tags('link').some(
    (element) => element.attributes.rel === 'canonical',
  )
  const crawlable = tags('a').every((link) => {
    const href = link.attributes.href ?? ''
    return href !== '' && !/^(#|javascript:)/.test(href)
  })

  lines.push(`title-length ${page.name} ${title.length}`)
  lines.push(
    Pages.check(
      'description',
      page.name,
      Boolean(meta('description')?.attributes.content),
    ),
  )
  lines.push(Pages.check('canonical', page.name, canonical))
  lines.push(Pages.check('viewport', page.name, meta('viewport') !== undefined))
  lines.push(Pages.check('one-h1', page.name, tags('h1').length === 1))
  lines.push(Pages.check('crawlable-links', page.name, crawlable))
}

Pages.write(lines)
