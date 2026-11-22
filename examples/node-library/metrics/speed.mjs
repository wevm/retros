/**
 * Writes the time to import each entry point in `package.json`. Each import runs in a fresh Node
 * process, so no module is cached.
 *
 * @module
 */

import { execFileSync } from 'node:child_process'
import { appendFileSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const pkg = JSON.parse(readFileSync('package.json', 'utf8'))

for (const [entry, file] of Object.entries(pkg.exports)) {
  const url = JSON.stringify(pathToFileURL(resolve(file)).href)
  const script = `const start = performance.now(); await import(${url}); process.stdout.write(String(performance.now() - start))`
  const ms = Number(execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }))

  appendFileSync(process.env.RETROS_METRICS, `import-time "${entry}" ${ms.toFixed(3)}ms\n`)
}
