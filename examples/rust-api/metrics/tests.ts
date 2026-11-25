/**
 * Runs `cargo test` and writes whether each test passes. A failing test is a measurement, so the
 * script still exits with status 0. The rust pack measures lints, formatting, and panics.
 *
 * @module
 */

import * as Server from './server.js'

const lines: string[] = []

// The test harness prints `test <name> ... ok` or `... FAILED` for each test.
const output = Bun.spawnSync([
  'cargo',
  'test',
  '--quiet',
  '--',
  '--format=pretty',
]).stdout.toString()
for (const [, name, result] of output.matchAll(
  /^test (\S+) \.\.\. (ok|FAILED)$/gm,
))
  lines.push(`passing ${name} ${result === 'ok' ? 'pass' : 'fail'}`)

Server.write(lines)
