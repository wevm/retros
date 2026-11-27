/**
 * Runs Lighthouse in its default mobile mode on every page for one category, named by the first
 * argument, and writes the results as metric lines. For performance, it writes Lighthouse's five
 * metrics and the bytes each page transfers. For every category, it writes whether each page
 * passes each audit that counts toward Lighthouse's score. Notes give Lighthouse's reasons: a
 * failing audit's title, such as "Image elements do not have `[alt]` attributes", the requests
 * that block rendering for the paint metrics, and the largest request for the bytes transferred.
 *
 * @module
 */

import * as fs from 'node:fs'

import * as ChromeLauncher from 'chrome-launcher'
import lighthouse from 'lighthouse'

import * as Serve from './serve.js'

/** Lighthouse audits written as metrics of the same name, with their units. */
const metrics = {
  'first-contentful-paint': 'ms',
  'largest-contentful-paint': 'ms',
  'total-blocking-time': 'ms',
  'cumulative-layout-shift': '',
  'speed-index': 'ms',
  'total-byte-weight': 'B',
} as const

const category = process.argv[2]!
const pages = fs.readdirSync('pages').filter((name) => name.endsWith('.html'))

const server = await Serve.start()
const chrome = await ChromeLauncher.launch({
  chromeFlags: ['--headless=new', '--no-sandbox'],
})
const lines: string[] = []

try {
  for (const page of pages) {
    const result = await lighthouse(`http://localhost:${server.port}/${page}`, {
      logLevel: 'error',
      onlyCategories: [category],
      port: chrome.port,
    })
    const report = result!.lhr

    if (category === 'performance') {
      const why: Record<string, string> = {}
      const blocking = report.audits['render-blocking-insight']
      const blockers = items<{ url: string }>(blocking)
      if (blockers.length > 0) {
        const urls = blockers.map((item) => new URL(item.url).pathname).join(', ')
        const savings = blocking!.displayValue ? `. ${blocking!.displayValue}` : ''
        for (const id of ['first-contentful-paint', 'largest-contentful-paint', 'speed-index'])
          why[id] = `Render-blocking requests: ${urls}${savings}`
      }
      const [largest] = items<{ totalBytes: number; url: string }>(report.audits['total-byte-weight'])
      if (largest)
        why['total-byte-weight'] = `Largest request: ${new URL(largest.url).pathname}, ${(largest.totalBytes / 1000).toFixed(1)} kB`

      for (const [id, unit] of Object.entries(metrics)) {
        const value = report.audits[id]!.numericValue!
        const note = why[id] ? ` note ${quote(why[id])}` : ''
        lines.push(
          `${id} ${page} ${unit === '' ? value.toFixed(3) : Math.round(value)}${unit}${note}`,
        )
      }
    }

    // An audit with weight counts toward Lighthouse's score, and passes at 0.9 or more.
    for (const reference of report.categories[category]!.auditRefs) {
      const audit = report.audits[reference.id]!
      if (reference.weight === 0 || reference.id in metrics || audit.score === null) continue
      if (audit.score >= 0.9) {
        lines.push(`audits "${page} ${reference.id}" pass`)
        continue
      }
      const note = [audit.title, audit.displayValue].filter(Boolean).join('. ')
      lines.push(`audits "${page} ${reference.id}" fail note ${quote(note)}`)
    }
  }
} finally {
  chrome.kill()
  server.stop(true)
}

fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Quotes a note for a metric line, escaping `"` and `\\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`
}

/** Returns the rows of an audit's details table, or none. */
function items<item>(audit: { details?: unknown } | undefined): readonly item[] {
  return (audit?.details as { items?: readonly item[] } | undefined)?.items ?? []
}
