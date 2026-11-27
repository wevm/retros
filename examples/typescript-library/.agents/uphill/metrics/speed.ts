/**
 * Writes the time to import each entry point in a fresh Bun process, and the median time each
 * formatter takes per call, as mitata measures it.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

import * as mitata from 'mitata'

import * as formatters from '../../../src/index.js'

const now = new Date('2026-11-22T12:00:00Z')

/** The name of each formatter in each namespace that the root entry point re-exports. */
type Name = {
  [Namespace in keyof typeof formatters]: keyof (typeof formatters)[Namespace]
}[keyof typeof formatters]

/** Arguments for one call of each formatter. A new export needs an entry here. */
const samples: Record<Name, readonly unknown[]> = {
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

// A module namespace lists its names in code-unit order. The flattened formatters keep that order.
const entries = Object.values(formatters)
  .flatMap((namespace) => Object.entries(namespace))
  .sort((a, b) => (a[0] < b[0] ? -1 : 1))
for (const [name, formatter] of entries) {
  const args = samples[name as Name]
  const call = formatter as (...args: readonly unknown[]) => unknown
  mitata.bench(name, () => call(...args))
}

const { benchmarks } = await mitata.run({ format: 'quiet' })
for (const benchmark of benchmarks)
  lines.push(
    `call-time ${benchmark.alias} ${benchmark.runs[0]!.stats!.p50.toFixed(1)}ns`,
  )

fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)
