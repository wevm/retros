#!/usr/bin/env node
/**
 * Runs the uphill binary for this machine, such as `binaries/linux-x64/uphill`, with the same
 * arguments, and exits with its status. Linux runs the static musl build, which needs no
 * particular C library, so one binary serves glibc and musl systems.
 * @module
 */
import * as childProcess from 'node:child_process'
import * as fs from 'node:fs'
import * as url from 'node:url'

const platform = `${process.platform}-${process.arch}`
const binary = url.fileURLToPath(
  new URL(`../binaries/${platform}/uphill`, import.meta.url),
)

// npm refuses other platforms through `os` and `cpu`, but `--force` and some package managers
// install anyway.
if (!fs.existsSync(binary)) {
  process.stderr.write(
    `error: uphill has no binary for ${platform}. Uphill has binaries for macOS and Linux on arm64 and x64.\n`,
  )
  process.exit(1)
}

const result = childProcess.spawnSync(binary, process.argv.slice(2), {
  stdio: 'inherit',
})
if (result.error) throw result.error

// A binary stopped by a signal stops this process with the same signal.
if (result.signal) process.kill(process.pid, result.signal)
process.exit(result.status ?? 1)
