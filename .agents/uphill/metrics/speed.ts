/**
 * Times the release binary: `uphill --version`, and `uphill check` on a one-category project.
 * Each value is the median of 20 runs, measured from Bun, so it includes the cost of starting a
 * process. Uphill runs the script 5 times and takes the median of those values.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

const runs = 20

run(['cargo', 'build', '--release', '--locked', '--quiet', '-p', 'uphill'])
const binary = path.resolve('target/release/uphill')

const project = fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-'))
fs.writeFileSync(
  path.join(project, 'uphill.yaml'),
  `one:\n  run: echo 'value 1' >> "$UPHILL_METRICS"\n  value: {}\n`,
)

try {
  const startup = medianMs([binary, '--version'])
  const check = medianMs([binary, 'check'], project)

  fs.appendFileSync(
    process.env.UPHILL_METRICS!,
    `startup ${startup.toFixed(3)}ms\ncheck ${check.toFixed(3)}ms\n`,
  )
} finally {
  fs.rmSync(project, { force: true, recursive: true })
}

/** Returns the median wall time of `command` over `runs` runs, in milliseconds. */
function medianMs(command: string[], cwd?: string): number {
  const samples: number[] = []

  for (let index = 0; index < runs; index++) {
    const start = Bun.nanoseconds()
    run(command, cwd)
    samples.push(Bun.nanoseconds() - start)
  }

  samples.sort((a, b) => a - b)
  return samples[Math.floor(samples.length / 2)]! / 1e6
}

/** Runs `command` and throws when it fails. */
function run(command: string[], cwd?: string): void {
  const result = Bun.spawnSync(command, {
    cwd,
    stderr: 'inherit',
    stdout: 'ignore',
  })
  if (result.exitCode !== 0)
    throw new Error(
      `${command.join(' ')} exited with status ${result.exitCode}`,
    )
}
