/**
 * Runs Lighthouse in its default mobile mode on every page for one category, named by the first
 * argument, and writes the results as metric lines. For performance, it writes Lighthouse's five
 * metrics and the bytes each page transfers. For every category, it writes whether each page
 * passes each audit that counts toward Lighthouse's score.
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
      for (const [id, unit] of Object.entries(metrics)) {
        const value = report.audits[id]!.numericValue!
        lines.push(`${id} ${page} ${unit === '' ? value.toFixed(3) : Math.round(value)}${unit}`)
      }
    }

    // An audit with weight counts toward Lighthouse's score, and passes at 0.9 or more.
    for (const reference of report.categories[category]!.auditRefs) {
      const audit = report.audits[reference.id]!
      if (reference.weight === 0 || reference.id in metrics || audit.score === null) continue
      const passed = audit.score >= 0.9 ? 'pass' : 'fail'
      lines.push(`audits "${page} ${reference.id}" ${passed}`)
    }
  }
} finally {
  chrome.kill()
  server.stop(true)
}

fs.appendFileSync(process.env.RETROS_METRICS!, `${lines.join('\n')}\n`)
