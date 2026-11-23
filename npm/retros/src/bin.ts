#!/usr/bin/env node
/**
 * Runs the retros binary from the platform package for this machine, such as
 * `@retros/linux-x64-gnu`, with the same arguments, and exits with its status.
 * npm installs only the platform package whose `os`, `cpu`, and `libc` match.
 * @module
 */
import * as childProcess from 'node:child_process'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)

const name = (() => {
  if (process.platform !== 'linux')
    return `@retros/${process.platform}-${process.arch}`

    // Node's diagnostic report names the glibc version, and names none on musl. Network details
    // can take seconds to collect, and @types/node does not declare the option that skips them.
  ;(process.report as { excludeNetwork?: boolean }).excludeNetwork = true
  const report = process.report.getReport() as {
    header?: { glibcVersionRuntime?: string | undefined } | undefined
  }
  const libc = report.header?.glibcVersionRuntime ? 'gnu' : 'musl'
  return `@retros/linux-${process.arch}-${libc}`
})()

const binary = (() => {
  try {
    return require.resolve(`${name}/retros`)
  } catch {
    process.stderr.write(
      `error: ${name} is not installed, so retros has no binary to run.\n` +
        'Retros has binaries for macOS and Linux on arm64 and x64. If npm omitted optional dependencies, reinstall retros with them.\n',
    )
    process.exit(1)
  }
})()

const result = childProcess.spawnSync(binary, process.argv.slice(2), {
  stdio: 'inherit',
})
if (result.error) throw result.error

// A binary stopped by a signal stops this process with the same signal.
if (result.signal) process.kill(process.pid, result.signal)
process.exit(result.status ?? 1)
