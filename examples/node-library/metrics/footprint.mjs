/**
 * Writes the gzipped size of each entry point in `package.json`, counting every module it imports
 * through a relative path.
 *
 * @module
 */

import { appendFileSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { gzipSync } from 'node:zlib'

const pkg = JSON.parse(readFileSync('package.json', 'utf8'))

for (const [entry, file] of Object.entries(pkg.exports)) {
  const source = [...modules(resolve(file))].map((path) => readFileSync(path, 'utf8')).join('\n')

  appendFileSync(process.env.RETROS_METRICS, `entry-size "${entry}" ${gzipSync(source).length}B\n`)
}

/** Returns a module and every module it imports through a relative path. */
function modules(path, seen = new Set()) {
  if (seen.has(path)) return seen
  seen.add(path)

  for (const [, specifier] of readFileSync(path, 'utf8').matchAll(/from '(\.[^']+)'/g))
    modules(join(dirname(path), specifier), seen)

  return seen
}
