/**
 * Writes the time to import each entry point in a fresh Bun process, and the median time each
 * formatter takes per call, as mitata measures it.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

import * as mitata from 'mitata'

import * as formatters from '../src/index.js'

const now = new Date('2026-11-22T12:00:00Z')

/** Arguments for one call of each formatter. A new export needs an entry here. */
const samples: Record<keyof typeof formatters, unknown[]> = {
  bytes: [45_600_000],
  grouped: [12_345],
  isoDate: [now],
  percent: [0.42],
  relative: [new Date('2026-11-19T12:00:00Z'), now],
  weekday: [now],
}

const pkg = await Bun.file('package.json').json()
const lines: string[] = []

for (const [entry, file] of Object.entries<string>(pkg.exports)) {
  const url = JSON.stringify(Bun.pathToFileURL(path.resolve(file)).href)
  const script = [
    'const start = performance.now()',
    `await import(${url})`,
    'console.write(String(performance.now() - start))',
  ].join('\n')
  const ms = Number(
    Bun.spawnSync([process.execPath, '-e', script]).stdout.toString(),
  )
  lines.push(`import-time "${entry}" ${ms.toFixed(3)}ms`)
}

for (const [name, formatter] of Object.entries(formatters)) {
  const args = samples[name as keyof typeof formatters]
  const call = formatter as (...args: unknown[]) => unknown
  mitata.bench(name, () => call(...args))
}

const { benchmarks } = await mitata.run({ format: 'quiet' })
for (const benchmark of benchmarks)
  lines.push(
    `call-time ${benchmark.alias} ${benchmark.runs[0]!.stats!.p50.toFixed(1)}ns`,
  )

fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)
