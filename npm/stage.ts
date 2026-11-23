/**
 * Prepares the npm packages for one release, in place: `bun npm/stage.ts <version> <archives>`.
 * Each platform package under `npm/platforms/<target>` gets the `retros` binary from
 * `<archives>/retros-<target>.tar.gz`, the Release workflow's archive for that target, and the
 * `retros` package depends on every platform package at the same version.
 *
 * @module
 */
import * as childProcess from 'node:child_process'
import * as fs from 'node:fs'
import * as path from 'node:path'

const [tag, archives] = process.argv.slice(2)
if (!tag || !archives)
  throw new Error('Usage: bun npm/stage.ts <version> <archives>')

const version = tag.replace(/^v/, '')
const root = path.join(import.meta.dirname, 'platforms')

const names = fs.readdirSync(root).map((target) => {
  const dir = path.join(root, target)
  const archive = path.resolve(archives, `retros-${target}.tar.gz`)

  childProcess.execFileSync('tar', ['-xzf', archive, '-C', dir, 'retros'])
  return write(path.join(dir, 'package.json'), { version })
})

write(path.join(import.meta.dirname, 'retros', 'package.json'), {
  optionalDependencies: Object.fromEntries(
    names.sort().map((name) => [name, version]),
  ),
  version,
})

/** Merges `fields` into the package.json at `file`, and returns the package name. */
function write(file: string, fields: Record<string, unknown>): string {
  const json = { ...JSON.parse(fs.readFileSync(file, 'utf8')), ...fields }
  fs.writeFileSync(file, `${JSON.stringify(json, null, 2)}\n`)
  return json.name
}
