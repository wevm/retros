/**
 * Runs Cargo's own tools and writes what they report: clippy's warnings in each source file, the
 * calls clippy's restriction lints find that can panic, rustfmt's verdict on each file, and
 * whether each test passes. A failing check is a measurement, so the script still exits with
 * status 0. A file clippy flags carries its first warning as the note.
 *
 * @module
 */

import * as path from 'node:path'

import * as Server from './server.js'

/** Restriction lints for calls that can panic. Clippy leaves them off by default. */
const panics = [
  'clippy::expect_used',
  'clippy::indexing_slicing',
  'clippy::panic',
  'clippy::todo',
  'clippy::unimplemented',
  'clippy::unreachable',
  'clippy::unwrap_used',
]

const sources = [...new Bun.Glob('src/**/*.rs').scanSync()].sort()
const lines: string[] = []

// Clippy writes one JSON message per line. Cargo replays the warnings of crates it already checked.
const warnings = run('cargo', [
  'clippy',
  '--quiet',
  '--message-format=json',
  '--',
  ...panics.flatMap((lint) => ['-W', lint]),
])
  .split('\n')
  .filter((line) => line.startsWith('{'))
  .map((line) => JSON.parse(line))
  .filter((message) => message.reason === 'compiler-message')
  .map((message) => {
    const span = message.message.spans.find(
      (span: { is_primary: boolean }) => span.is_primary,
    )
    return {
      code: message.message.code?.code as string | undefined,
      file: span?.file_name as string | undefined,
      line: span?.line_start as number | undefined,
      text: message.message.message as string,
    }
  })
  .filter((warning) => warning.file)

for (const file of sources) {
  const warning = warnings.find(
    (warning) => warning.file === file && !panics.includes(warning.code!),
  )
  const note = `line ${warning?.line}: ${warning?.text}`
    .replaceAll('\\', '\\\\')
    .replaceAll('"', '\\"')
  lines.push(
    warning ? `clippy ${file} fail note "${note}"` : `clippy ${file} pass`,
  )
}
lines.push(
  `panic-sites ${warnings.filter((warning) => panics.includes(warning.code!)).length}`,
)

// `cargo fmt --check -- -l` lists the absolute path of each file rustfmt would change.
const unformatted = run('cargo', ['fmt', '--check', '--', '-l'])
  .split('\n')
  .filter(Boolean)
  .map((file) => path.relative(process.cwd(), file))
for (const file of sources)
  lines.push(
    `formatted ${file} ${unformatted.includes(file) ? 'fail' : 'pass'}`,
  )

// The test harness prints `test <name> ... ok` or `... FAILED` for each test.
const tests = run('cargo', ['test', '--quiet', '--', '--format=pretty'])
for (const [, name, result] of tests.matchAll(
  /^test (\S+) \.\.\. (ok|FAILED)$/gm,
))
  lines.push(`tests ${name} ${result === 'ok' ? 'pass' : 'fail'}`)

Server.write(lines)

/** Runs a command and returns its standard output. A failing exit status is expected. */
function run(command: string, args: string[]): string {
  return Bun.spawnSync([command, ...args]).stdout.toString()
}
