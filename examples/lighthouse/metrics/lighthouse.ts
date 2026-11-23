/**
 * Runs Lighthouse on every page in `pages/` and writes each audit that counts toward a category's
 * score as a metric line, with the page as its subject, so Uphill scores each page the way
 * Lighthouse does.
 *
 * The first argument is the form factor, `mobile` or `desktop`, and the rest are Lighthouse
 * category ids, such as `bun metrics/lighthouse.ts mobile accessibility best-practices`:
 *
 * - **Metrics:** a metric audit, such as `largest-contentful-paint`, writes its numeric value, and
 *   `uphill.yaml` holds Lighthouse's control points as `target` and `poor`. Its note lists the
 *   insights that would save time on it, as Lighthouse's metric filter does.
 * - **Binary audits:** an audit such as `image-alt` writes `pass` or `fail`, with the failure
 *   title and the elements that fail as its note.
 * - **Not applicable:** an audit with nothing to audit, or a manual or informative one, writes
 *   `skip`, so it leaves the category like Lighthouse sets its weight to 0.
 * - **Fractions:** a category that Lighthouse shows as a fraction, such as `agentic-browsing`,
 *   writes each audit as `pass` when Lighthouse shows it as passed.
 *
 * An audit that errors makes Lighthouse's category score null, so the script exits with status 1
 * and Uphill gives the category no score.
 *
 * When `LIGHTHOUSE_REPORTS` names a directory, each page's report is read from
 * `<directory>/<page>/<form factor>-<categories>.json` when it exists, and written there after
 * Lighthouse runs. `metrics/verify.ts` compares Uphill's scores with the reports there.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

import * as ChromeLauncher from 'chrome-launcher'
import lighthouse from 'lighthouse'
import defaultConfig from 'lighthouse/core/config/default-config.js'
import desktopConfig from 'lighthouse/core/config/desktop-config.js'

import * as Serve from './serve.js'

/** The parts of a Lighthouse result that the metric lines come from. */
export type Report = {
  audits: Record<string, Audit>
  categories: Record<string, { auditRefs: readonly { group?: string | undefined; id: string; weight: number }[]; categoryScoreDisplayMode?: string | undefined; score: number | null }>
  configSettings: { formFactor: string }
  lighthouseVersion: string
}

/** One audit result, with the details its note needs. */
type Audit = {
  displayValue?: string | undefined
  errorMessage?: string | undefined
  metricSavings?: Record<string, number> | undefined
  nodes?: readonly string[] | undefined
  numericUnit?: string | undefined
  numericValue?: number | undefined
  score: number | null
  scoreDisplayMode: string
  title: string
}

/** The acronyms Lighthouse files metric savings under, by metric audit. */
const acronyms: Record<string, string> = {
  'cumulative-layout-shift': 'CLS',
  'first-contentful-paint': 'FCP',
  'largest-contentful-paint': 'LCP',
  'speed-index': 'SI',
  'total-blocking-time': 'TBT',
}

if (import.meta.main) {
  const [formFactor, ...categories] = process.argv.slice(2)
  if ((formFactor !== 'mobile' && formFactor !== 'desktop') || categories.length === 0)
    throw new Error('usage: bun metrics/lighthouse.ts <mobile|desktop> <category>...')

  const directory = process.env.LIGHTHOUSE_REPORTS
  const pages = fs.readdirSync('pages').filter((name) => name.endsWith('.html')).sort()
  const file = (page: string) => path.join(directory ?? '', page, `${formFactor}-${categories.join('+')}.json`)
  const missing = pages.filter((page) => !directory || !fs.existsSync(file(page)))

  const reports: Record<string, Report> = {}
  for (const page of pages.filter((page) => !missing.includes(page)))
    reports[page] = JSON.parse(fs.readFileSync(file(page), 'utf8'))

  if (missing.length > 0) {
    const server = await Serve.start()
    const chrome = await ChromeLauncher.launch({ chromeFlags: ['--headless=new', '--no-sandbox'] })
    try {
      for (const page of missing) {
        const result = await lighthouse(
          `http://localhost:${server.port}/${page}`,
          { logLevel: 'error', onlyCategories: categories, port: chrome.port },
          formFactor === 'desktop' ? desktopConfig : undefined,
        )
        reports[page] = trim(result!.lhr)
        if (directory) {
          fs.mkdirSync(path.dirname(file(page)), { recursive: true })
          fs.writeFileSync(file(page), `${JSON.stringify(reports[page], null, 1)}\n`)
        }
      }
    } finally {
      chrome.kill()
      server.stop(true)
    }
  }

  const lines = pages.flatMap((page) => categories.flatMap((category) => metrics(page, reports[page]!, category)))
  fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)
}

/**
 * Returns the metric lines for one category of a page's report: one line for each audit that
 * Lighthouse's default configuration weighs above 0.
 */
export function metrics(page: string, report: Report, category: string): string[] {
  const fraction = report.categories[category]?.categoryScoreDisplayMode === 'fraction'
  const references = defaultConfig.categories![category]!.auditRefs.filter((reference) => reference.weight > 0)

  return references.map((reference) => {
    const audit = report.audits[reference.id]!
    const line = (value: string, note?: string) => `${page}: ${reference.id} ${value}${note ? ` note ${quote(note)}` : ''}`

    if (['informative', 'manual', 'notApplicable'].includes(audit.scoreDisplayMode)) return line('skip')

    // Lighthouse's report shows an audit as passed at a score of 0.9 or more, and counts an audit
    // that errors as failed.
    if (fraction) return audit.score !== null && audit.score >= 0.9 ? line('pass') : line('fail', failure(audit))

    if (audit.scoreDisplayMode === 'error') {
      process.stderr.write(`${page}: ${reference.id} errored: ${audit.errorMessage}\n`)
      process.exitCode = 1
      return line('fail', failure(audit))
    }

    const acronym = acronyms[reference.id]
    if (acronym && audit.numericValue !== undefined) {
      const value = decimal(audit.numericValue)
      return line(audit.numericUnit === 'millisecond' ? `${value}ms` : value, savings(report, acronym))
    }

    if (audit.scoreDisplayMode !== 'binary')
      throw new Error(`${reference.id} scores in ${audit.scoreDisplayMode} mode, which this script does not write`)
    return audit.score === 1 ? line('pass') : line('fail', failure(audit))
  })
}

/** Returns the note for a failed audit: its title, display value, and the first failing elements. */
function failure(audit: Audit): string {
  const nodes = audit.nodes ?? []
  const more = nodes.length > 3 ? `, and ${nodes.length - 3} more` : ''
  const elements = nodes.length > 0 ? `: ${nodes.slice(0, 3).join(', ')}${more}` : ''
  return [audit.title, audit.displayValue].filter(Boolean).join('. ') + elements
}

/** Returns the note for a metric: the insights that estimate savings on it, largest first. */
function savings(report: Report, acronym: string): string | undefined {
  const insights = Object.values(report.audits)
    .filter((audit) => (audit.metricSavings?.[acronym] ?? 0) > 0 && audit.score !== 1)
    .sort((a, b) => b.metricSavings![acronym]! - a.metricSavings![acronym]!)
  if (insights.length === 0) return undefined
  return insights.map((audit) => [audit.title, audit.displayValue].filter(Boolean).join(', ')).join('. ')
}

/** Keeps the parts of a Lighthouse result that the metric lines come from. */
function trim(result: Report & { audits: Record<string, Audit & { details?: unknown }> }): Report {
  const audits: Record<string, Audit> = {}
  for (const [id, audit] of Object.entries(result.audits)) {
    // Accessibility audits list each element that fails as a table row with a node.
    const items = (audit.details as { items?: unknown } | undefined)?.items
    const rows = Array.isArray(items) ? (items as readonly { node?: { selector?: string } }[]) : []
    const nodes = rows.flatMap((row) => (row.node?.selector ? [row.node.selector] : []))
    audits[id] = {
      displayValue: audit.displayValue,
      errorMessage: audit.errorMessage,
      metricSavings: audit.metricSavings,
      nodes: nodes.length > 0 ? nodes : undefined,
      numericUnit: audit.numericUnit,
      numericValue: audit.numericValue,
      score: audit.score,
      scoreDisplayMode: audit.scoreDisplayMode,
      title: audit.title,
    }
  }

  const categories: Report['categories'] = {}
  for (const [id, category] of Object.entries(result.categories)) {
    categories[id] = {
      auditRefs: category.auditRefs.map((reference) => ({ group: reference.group, id: reference.id, weight: reference.weight })),
      categoryScoreDisplayMode: category.categoryScoreDisplayMode,
      score: category.score,
    }
  }

  return { audits, categories, configSettings: { formFactor: result.configSettings.formFactor }, lighthouseVersion: result.lighthouseVersion }
}

/** Writes a number as a decimal without an exponent, such as `0.0000012` for `1.2e-6`. */
function decimal(value: number): string {
  const text = String(value)
  if (!text.includes('e')) return text
  return value.toFixed(20).replace(/0+$/, '')
}

/** Quotes a note for a metric line, escaping `"` and `\\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`
}
