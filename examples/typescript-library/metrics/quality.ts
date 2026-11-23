/**
 * Runs the tools the library already uses and writes what they report: Bun's test runner for
 * tests and coverage, TypeScript for type errors, oxlint for lint, and oxfmt for formatting. A
 * failing check is a measurement, so the script still exits with status 0. Each miss carries the
 * tool's own reason as its note.
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

// One `<testcase>` per test. A failed test holds a `<failure>` element with a message.
const junit = fs.readFileSync(path.join(scratch, 'junit.xml'), 'utf8')
for (const [, attributes, body = ''] of junit.matchAll(
  /<testcase ([^>]*?)(?:\/>|>([\s\S]*?)<\/testcase>)/g,
)) {
  const name = unescape(/name="([^"]*)"/.exec(attributes!)![1]!)
  const file = path.basename(/file="([^"]*)"/.exec(attributes!)![1]!)
  const failure = /<failure(?: message="([^"]*)")?/.exec(body)
  lines.push(
    `tests ${quote(`${file} > ${name}`)} ${failure ? 'fail' : 'pass'}${note(failure?.[1] && unescape(failure[1]))}`,
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
  // `DA:<line>,0` marks a line that no test runs.
  const missed = [...record.matchAll(/^DA:(\d+),0$/gm)].map((match) => Number(match[1]))
  lines.push(
    `coverage ${file} ${found === 0 ? 100 : ((hit / found) * 100).toFixed(1)}${note(missed.length > 0 && `no test runs ${ranges(missed)}`)}`,
  )
}

// TypeScript prints one `file(line,column): error TS…` line per error.
const types = run('node_modules/.bin/tsc', ['-p', '.', '--pretty', 'false'])
for (const file of sources) {
  const error = types
    .split('\n')
    .map((line) => /^(.+)\((\d+),\d+\): error (.+)$/.exec(line))
    .find((match) => match?.[1] === file)
  lines.push(
    `types ${file} ${error ? 'fail' : 'pass'}${note(error && `line ${error[2]}: ${error[3]}`)}`,
  )
}

const lint: {
  diagnostics: {
    filename: string
    labels: { span: { line: number } }[]
    message: string
  }[]
} = JSON.parse(run('node_modules/.bin/oxlint', ['--format=json', 'src']))
for (const file of sources) {
  const diagnostic = lint.diagnostics.find(
    (diagnostic) => diagnostic.filename === file,
  )
  const line = diagnostic?.labels[0]?.span.line
  lines.push(
    `lint ${file} ${diagnostic ? 'fail' : 'pass'}${note(diagnostic && `line ${line}: ${diagnostic.message}`)}`,
  )
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
  const error = await import(path.resolve(file)).then(
    () => undefined,
    (error: Error) => error.message,
  )
  lines.push(
    `exports-resolve "${entry}" ${error ? 'fail' : 'pass'}${note(error)}`,
  )
}

fs.rmSync(scratch, { force: true, recursive: true })
fs.appendFileSync(process.env.RETROS_METRICS!, `${lines.join('\n')}\n`)

/** Runs a command and returns its standard output. A failing exit status is expected. */
function run(command: string, args: string[]): string {
  return Bun.spawnSync([command, ...args]).stdout.toString()
}

/** Quotes an item or note for a metric line, escaping `"` and `\`. */
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

/** Returns the end of a metric line that says why, such as ` note "line 8: …"`, or nothing. */
function note(text: string | false | undefined): string {
  return text ? ` note ${quote(text.split('\n')[0]!)}` : ''
}

/** Formats line numbers as ranges, such as `lines 3-7 and 12`. */
function ranges(numbers: number[]): string {
  const parts: string[] = []
  for (let start = 0; start < numbers.length; ) {
    let end = start
    while (numbers[end + 1] === numbers[end]! + 1) end++
    parts.push(
      end === start ? `${numbers[start]}` : `${numbers[start]}-${numbers[end]}`,
    )
    start = end + 1
  }
  const list =
    parts.length > 1
      ? `${parts.slice(0, -1).join(', ')} and ${parts.at(-1)}`
      : parts[0]
  return `${numbers.length === 1 ? 'line' : 'lines'} ${list}`
}
