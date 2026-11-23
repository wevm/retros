/**
 * Sets the version of the Rust workspace to the version of the `uphill` npm package, so
 * `uphill --version` matches the npm version and the GitHub Release tag. `pnpm changeset:version`
 * runs it after `changeset version` raises the package version: `bun npm/version.ts`.
 * @module
 */
import * as fs from 'node:fs'
import * as path from 'node:path'

const root = path.join(import.meta.dirname, '..')

const { version } = JSON.parse(
  fs.readFileSync(path.join(import.meta.dirname, 'package.json'), 'utf8'),
) as { version: string }

// `[workspace.package]` holds the only `version` key at the start of a line in Cargo.toml.
// Cargo.lock lists workspace crates without a `source` line, unlike crates from a registry.
const edits = [
  { file: 'Cargo.toml', pattern: /^version = "[^"]*"$/m },
  {
    file: 'Cargo.lock',
    pattern: /^(name = "[^"]+"\nversion = )"[^"]*"$(?!\nsource)/gm,
  },
] as const

for (const edit of edits) {
  const file = path.join(root, edit.file)
  const text = fs.readFileSync(file, 'utf8')
  if (!text.match(edit.pattern))
    throw new Error(`${edit.file} has no version to set.`)

  fs.writeFileSync(
    file,
    text.replace(edit.pattern, (match) =>
      match.replace(/"[^"]*"$/, `"${version}"`),
    ),
  )
}
