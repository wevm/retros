/**
 * Compares the scores in `.uphill/report.json` with the Lighthouse reports that
 * `metrics/lighthouse.ts` saved under `LIGHTHOUSE_REPORTS`, and exits with status 1 when any
 * category or audit score differs. Run `uphill` with the same `LIGHTHOUSE_REPORTS` first, so
 * every round reads the same reports and each median equals the reported value.
 *
 * A category named `<id>-desktop` compares with Lighthouse's desktop report for `<id>`. A
 * category that Lighthouse shows as a fraction compares with that fraction as a percentage, from
 * Lighthouse's own `ReportUtils.calculateCategoryFraction`.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

import { ReportUtils } from 'lighthouse/report/renderer/report-utils.js'

import type { Report } from './lighthouse.js'

/** A category of `.uphill/report.json`. */
type Scored = {
  metrics?: readonly { name: string; score: number | null; value: number | null }[] | undefined
  name: string
  score: number | null
  subject: string | null
}

const directory = process.env.LIGHTHOUSE_REPORTS
if (!directory) throw new Error('set LIGHTHOUSE_REPORTS to the directory `uphill` ran with')

const scored: readonly Scored[] = JSON.parse(fs.readFileSync('.uphill/report.json', 'utf8')).categories
let audits = 0
let mismatches = 0

for (const category of scored) {
  // A run that fails has no subjects, since Uphill reads no lines from it.
  if (category.subject === null) {
    mismatches++
    console.log(`✗ ${category.name} failed`)
    continue
  }

  const page = category.subject
  const desktop = category.name.endsWith('-desktop')
  const id = category.name.replace(/-desktop$/, '')
  const report = reports(page).find((report) => report.configSettings.formFactor === (desktop ? 'desktop' : 'mobile') && id in report.categories)
  if (!report) throw new Error(`no ${desktop ? 'desktop' : 'mobile'} report for ${page} has ${id}`)

  const expected = (() => {
    const lighthouse = report.categories[id]!
    if (lighthouse.categoryScoreDisplayMode !== 'fraction') return lighthouse.score === null ? null : Math.round(lighthouse.score * 100)
    const auditRefs = lighthouse.auditRefs.map((reference) => ({ ...reference, result: report.audits[reference.id] }))
    const fraction = ReportUtils.calculateCategoryFraction({ ...lighthouse, auditRefs } as never)
    return Math.round((fraction.numPassed / fraction.numPassableAudits) * 100)
  })()

  const same = expected === category.score
  if (!same) mismatches++
  console.log(`${same ? '✓' : '✗'} ${page.padEnd(14)} ${category.name.padEnd(20)} Lighthouse ${String(expected).padStart(3)}  Uphill ${String(category.score).padStart(3)}`)

  // Each audit with a score scores the same in both, except in a category shown as a fraction.
  if (report.categories[id]!.categoryScoreDisplayMode === 'fraction') continue
  for (const metric of category.metrics ?? []) {
    const audit = report.audits[metric.name]!
    // Lighthouse gives an informative, manual, or not applicable audit no weight.
    const weightless = ['informative', 'manual', 'notApplicable'].includes(audit.scoreDisplayMode)
    const expected = audit.score === null || weightless ? null : Math.round(audit.score * 100)
    audits++
    if (expected === metric.score) continue
    mismatches++
    console.log(`  ✗ ${metric.name}: Lighthouse ${expected}, Uphill ${metric.score}`)
  }
}

console.log(`\n${scored.length} category scores and ${audits} audit scores compared, ${mismatches} differ`)
if (mismatches > 0) process.exitCode = 1

/** Returns every report saved for a page. */
function reports(page: string): Report[] {
  const folder = path.join(directory!, page)
  return fs.readdirSync(folder).map((name) => JSON.parse(fs.readFileSync(path.join(folder, name), 'utf8')))
}
