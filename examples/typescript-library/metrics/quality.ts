/**
 * Runs the tools the library already uses and writes what they report: Bun's test runner for
 * tests and coverage, TypeScript for type errors, oxlint for lint, and oxfmt for formatting. A
 * failing check is a measurement, so the script still exits with status 0.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

const pkg = await Bun.file('package.json').json()
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'quality-'))
const sources = [...new Bun.Glob('src/**/*.ts').scanSync()].sort()
const lines: string[] = []

run('bun', [
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

// TypeScript prints one `file(line,column): error TS…` line per error.
const types = run('node_modules/.bin/tsc', ['-p', '.', '--pretty', 'false'])
for (const file of sources)
  lines.push(`types ${file} ${types.includes(`${file}(`) ? 'fail' : 'pass'}`)

const lint: { diagnostics: { filename: string }[] } = JSON.parse(
  run('node_modules/.bin/oxlint', ['--format=json', 'src']),
)
for (const file of sources) {
  const failed = lint.diagnostics.some(
    (diagnostic) => diagnostic.filename === file,
  )
  lines.push(`lint ${file} ${failed ? 'fail' : 'pass'}`)
}

// oxfmt lists each file whose formatting differs, one per line.
const unformatted = run('node_modules/.bin/oxfmt', ['--list-different', 'src'])
  .split('\n')
  .map((line) => line.trim())
for (const file of sources)
  lines.push(
    `formatted ${file} ${unformatted.includes(file) ? 'fail' : 'pass'}`,
  )

for (const [entry, file] of Object.entries<string>(pkg.exports)) {
  const resolves = await import(path.resolve(file)).then(
    () => true,
    () => false,
  )
  lines.push(`exports-resolve "${entry}" ${resolves ? 'pass' : 'fail'}`)
}

fs.rmSync(scratch, { force: true, recursive: true })
fs.appendFileSync(process.env.RETROS_METRICS!, `${lines.join('\n')}\n`)

/** Runs a command and returns its standard output. A failing exit status is expected. */
function run(command: string, args: string[]): string {
  return Bun.spawnSync([command, ...args]).stdout.toString()
}

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
