/**
 * Writes the score gauges that the GitHub Action links in its pull request comment: one SVG per
 * score from 0 to 100 and one for a failed category, in the colors of the HTML report. GitHub
 * renders only images that it fetches by URL, so the gauges are committed files. Run it with
 * `bun assets/gauges.ts` after a change to the drawing.
 * @module
 */
import * as fs from 'node:fs'
import * as path from 'node:path'
import * as url from 'node:url'

const directory = url.fileURLToPath(new URL('./gauges/', import.meta.url))

// Lighthouse's bands, with the light and dark text colors of the HTML report.
const bands = {
  average: {
    arc: '#fa3',
    dark: '#ffa400',
    fill: '255,170,51',
    light: '#c33300',
  },
  good: { arc: '#0c6', dark: '#0c6', fill: '0,204,102', light: '#008800' },
  none: {
    arc: '#9e9e9e',
    dark: '#9aa0a6',
    fill: '158,158,158',
    light: '#5f6368',
  },
  poor: { arc: '#f33', dark: '#ff4e42', fill: '255,51,51', light: '#c00' },
} as const

/** Returns the gauge for `score`, or the gauge of a failed category when `score` is `null`. */
function gauge(score: number | null) {
  const band = (() => {
    if (score === null) return bands.none
    if (score >= 90) return bands.good
    if (score >= 50) return bands.average
    return bands.poor
  })()

  // A round cap draws even an arc of length zero, so a score of 0 and a failure draw no arc.
  const arc = score
    ? `<circle cx="60" cy="60" r="52" fill="none" stroke="${band.arc}" stroke-width="8" stroke-linecap="round" pathLength="100" stroke-dasharray="${score} 100" transform="rotate(-90 60 60)"/>`
    : ''
  const label = score === null ? 'Failed' : `Score ${score}`
  const text = score === null ? '✗' : String(score)

  return `<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="0 0 120 120" role="img" aria-label="${label}">
<style>text{fill:${band.light}}@media (prefers-color-scheme:dark){text{fill:${band.dark}}}</style>
<circle cx="60" cy="60" r="52" fill="rgba(${band.fill},.14)" stroke="rgba(128,128,128,.22)" stroke-width="8"/>
${arc}<text x="60" y="61" text-anchor="middle" dominant-baseline="central" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif" font-size="36" font-weight="600">${text}</text>
</svg>
`
}

fs.rmSync(directory, { force: true, recursive: true })
fs.mkdirSync(directory, { recursive: true })

for (let score = 0; score <= 100; score++)
  fs.writeFileSync(path.join(directory, `${score}.svg`), gauge(score))
fs.writeFileSync(path.join(directory, 'failed.svg'), gauge(null))
