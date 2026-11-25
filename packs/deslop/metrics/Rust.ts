/** @module Identifies Rust source inputs and reads deslop-please's diagnostics for the same Cargo workspace. */
import * as fs from 'node:fs'
import * as path from 'node:path'

/** Runs the production build with Cargo's configured target and default features. */
export function workspace() {
  if (!fs.existsSync('Cargo.toml')) return undefined
  const files = new Set<string>()
  const owners = new Map<string, Set<string>>()
  const backend = Bun.which('deslop-please') ? 'deslop-please' : 'scanner'
  const label = `${backend}, Cargo default features and configured target`
  if (!Bun.which('cargo'))
    return {
      files,
      label,
      owners,
      reason: 'Cargo unavailable',
      diagnostics: undefined,
    }

  const meta = JSON.parse(
    run(['cargo', 'metadata', '--no-deps', '--format-version=1']),
  ) as {
    target_directory: string
    packages: { name: string; targets: { name: string; kind: string[] }[] }[]
    workspace_members: string[]
    workspace_root: string
  }
  const build = run([
    'cargo',
    'check',
    '--workspace',
    '--message-format=json-render-diagnostics',
  ])
  for (const line of build.trim().split('\n')) {
    const event = JSON.parse(line) as {
      filenames: string[]
      package_id: string
      profile: { test: boolean }
      reason: string
      target: { src_path: string }
    }
    if (
      event.reason !== 'compiler-artifact' ||
      !meta.workspace_members.includes(event.package_id) ||
      event.profile.test
    )
      continue
    files.add(path.relative('.', event.target.src_path))

    // Only read dep-info paired with an artifact Cargo returned in this run, including fresh artifacts.
    for (const output of event.filenames) {
      const name = path.basename(output).match(/^lib(.+)-([\da-f]+)\.rmeta$/)
      if (!name) continue
      const depInfo = path.join(path.dirname(output), `${name[1]}-${name[2]}.d`)
      if (!fs.existsSync(depInfo)) continue
      for (const match of fs
        .readFileSync(depInfo, 'utf8')
        .matchAll(/^(.+\.rs):\r?$/gm)) {
        const file = match[1]!.replaceAll('\\ ', ' ')
        files.add(path.relative('.', path.resolve(meta.workspace_root, file)))
      }
    }
  }

  if (backend === 'scanner')
    return {
      files,
      label,
      owners,
      reason: 'not a verified Cargo production input',
      diagnostics: undefined,
    }

  const allow = '.deslop-please.allow'
  const diagnostics = run(
    ['deslop-please', ...(fs.existsSync(allow) ? ['--allow', allow] : [])],
    true,
  )
  const reason =
    'not verified in both Cargo production inputs and the deslop-please index'
  // A build input can be data (include_str!), not a module. Require a SCIP document too.
  // If different default-feature indexes share this Cargo target directory, do not guess which ran.
  const cache = path.join(meta.target_directory, 'deslop-please')
  const indexes = fs.existsSync(cache)
    ? [
        ...new Bun.Glob('*/ra-config.json').scanSync({
          cwd: cache,
          absolute: true,
        }),
      ].filter((file) => fs.readFileSync(file, 'utf8').trim() === '{}')
    : []
  const index =
    indexes.length === 1
      ? path.join(path.dirname(indexes[0]!), 'index.scip')
      : undefined
  const indexed = new Set<string>()
  if (index && fs.existsSync(index)) {
    // The field owner in deslop's report is a qualified Rust name. SCIP's document.symbols
    // lists definitions, not references, so it identifies the declaring file even for #[path].
    const crates = new Map(
      meta.packages.map((pkg) => [
        pkg.name,
        pkg.targets.find((t) =>
          t.kind.some((kind) => kind.includes('lib') || kind === 'proc-macro'),
        )?.name ?? pkg.name.replaceAll('-', '_'),
      ]),
    )
    for (const document of bytesFields(fs.readFileSync(index), 2)) {
      const name = bytesFields(document, 1).next().value
      if (!name) continue
      const file = path.relative(
        '.',
        path.resolve(meta.workspace_root, name.toString()),
      )
      if (!files.has(file)) continue
      indexed.add(file)
      for (const definition of bytesFields(document, 3)) {
        const symbol = bytesFields(definition, 1).next().value?.toString()
        const type =
          symbol && /^rust-analyzer cargo ([\w-]+) \S+ ([\w/]+)#$/.exec(symbol)
        if (!type || !crates.has(type[1]!)) continue
        const owner = `${crates.get(type[1]!)}::${type[2]!.replaceAll('/', '::')}`
        if (!owners.has(owner)) owners.set(owner, new Set())
        owners.get(owner)!.add(file)
      }
    }
  }
  for (const file of files) if (!indexed.has(file)) files.delete(file)
  return { diagnostics, files, label, owners, reason }
}

/** Yields the specified length-delimited protobuf field without decoding occurrences or symbols. */
function* bytesFields(data: Buffer, field: number): Generator<Buffer> {
  let offset = 0
  const varint = () => {
    let value = 0
    for (let shift = 0; shift < 53 && offset < data.length; shift += 7) {
      const byte = data[offset++]!
      value += (byte & 127) * 2 ** shift
      if (byte < 128) return value
    }
    throw Error('invalid SCIP varint')
  }
  while (offset < data.length) {
    const tag = varint()
    switch (tag % 8) {
      case 0:
        varint()
        break
      case 1:
        offset += 8
        break
      case 2: {
        const size = varint()
        const start = offset
        offset += size
        if (Math.floor(tag / 8) === field) yield data.subarray(start, offset)
        break
      }
      case 5:
        offset += 4
        break
      default:
        throw Error('unsupported SCIP wire type')
    }
    if (offset > data.length) throw Error('incomplete SCIP index')
  }
}

/** A native finding is exit 1. Every other unsuccessful invocation is an analysis failure. */
function run(args: string[], native = false): string {
  const result = Bun.spawnSync(args, {
    env: { ...process.env, CARGO_TERM_COLOR: 'never' },
    maxBuffer: 64 * 1024 * 1024,
  })
  const output = result.stdout.toString()
  if (
    native
      ? (result.exitCode === 0 &&
          /\ndeslop-please: ok\s*$/.test(`\n${output}`)) ||
        (result.exitCode === 1 && /^deslop-please: \d+ errors?,/m.test(output))
      : result.success
  )
    return output
  throw Error(`${args.join(' ')}: ${result.stderr.toString() || output}`)
}
