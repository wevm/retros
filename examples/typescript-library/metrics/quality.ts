/**
 * Writes whether each test passes, the share of lines tests cover in each source file, whether
 * each export has a TSDoc comment, and two packaging checks. A failing test is a measurement, so
 * the script still exits with status 0.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

const pkg = await Bun.file('package.json').json()
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'quality-'))
const lines: string[] = []

Bun.spawnSync([
  'bun',
  'test',
  '--reporter=junit',
  `--reporter-outfile=${path.join(scratch, 'junit.xml')}`,
  '--coverage',
  '--coverage-reporter=lcov',
  `--coverage-dir=${scratch}`,
])

// One `<testcase>` per test. A failed test holds a `<failure>` element.
const junit = fs.readFileSync(path.join(scratch, 'junit.xml'), 'utf8')
for (const [, attributes, body = ''] of junit.matchAll(
  /<testcase ([^>]*?)(?:\/>|>([\s\S]*?)<\/testcase>)/g,
)) {
  const name = unescape(/name="([^"]*)"/.exec(attributes!)![1]!)
  const file = path.basename(/file="([^"]*)"/.exec(attributes!)![1]!)
  lines.push(
    `tests ${quote(`${file} > ${name}`)} ${body.includes('<failure') ? 'fail' : 'pass'}`,
  )
}

// lcov records `LF` lines found and `LH` lines hit for each source file.
const lcov = fs.readFileSync(path.join(scratch, 'lcov.info'), 'utf8')
for (const record of lcov
  .split('end_of_record')
  .filter((record) => record.includes('SF:'))) {
  const file = /^SF:(.+)$/m.exec(record)![1]!
  const found = Number(/^LF:(\d+)$/m.exec(record)![1])
  const hit = Number(/^LH:(\d+)$/m.exec(record)![1])
  lines.push(
    `coverage ${file} ${found === 0 ? 100 : ((hit / found) * 100).toFixed(1)}`,
  )
}

for (const file of new Bun.Glob('src/**/*.ts').scanSync()) {
  if (file.endsWith('.test.ts')) continue

  const source = fs.readFileSync(file, 'utf8')
  for (const [, doc, name] of source.matchAll(
    /(\/\*\*[\s\S]*?\*\/\s*)?export (?:async )?(?:function|const|class) (\w+)/g,
  )) {
    lines.push(
      `documented ${path.basename(file)}:${name} ${doc ? 'pass' : 'fail'}`,
    )
  }
}

lines.push(`side-effects ${pkg.sideEffects === false ? 'pass' : 'fail'}`)

for (const [entry, file] of Object.entries<string>(pkg.exports)) {
  const resolves = await import(path.resolve(file)).then(
    () => true,
    () => false,
  )
  lines.push(`exports-resolve "${entry}" ${resolves ? 'pass' : 'fail'}`)
}

fs.rmSync(scratch, { force: true, recursive: true })
fs.appendFileSync(process.env.RETROS_METRICS!, `${lines.join('\n')}\n`)

/** Quotes an item for a metric line, escaping `"` and `\`. */
function quote(item: string): string {
  return `"${item.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`
}

/** Replaces the XML entities that JUnit names use. */
function unescape(text: string): string {
  return text
    .replaceAll('&quot;', '"')
    .replaceAll('&apos;', "'")
    .replaceAll('&lt;', '<')
    .replaceAll('&gt;', '>')
    .replaceAll('&amp;', '&')
}
