/** @module Checks Cargo's resolved package settings and the conventions recorded in workspace manifests. */
import * as fs from 'node:fs'
import * as Cargo from './Cargo.js'

/** Clippy lints the conventions allow workspace-wide. */
export const allowed = [
  'module_name_repetitions',
  'must_use_candidate',
  'similar_names',
  'too_many_lines',
]

const mockCrates = new Set([
  'faux',
  'mockall',
  'mockito',
  'mocktopus',
  'wiremock',
])
const restrictions = [
  'allow_attributes',
  'dbg_macro',
  'get_unwrap',
  'iter_over_hash_type',
  'print_stderr',
  'print_stdout',
  'todo',
  'undocumented_unsafe_blocks',
  'unimplemented',
]

type Table = Record<string, unknown>
function table(value: unknown): Table {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Table)
    : {}
}

function read(file: string): Table {
  return table(Bun.TOML.parse(fs.readFileSync(file, 'utf8')))
}

function lintTable(value: unknown): Map<string, unknown> {
  return new Map(
    Object.entries(table(value)).map((entry) => [
      entry[0].replaceAll('-', '_'),
      table(entry[1]).level ?? entry[1],
    ]),
  )
}

/** A single check; an unknown result has a reason and does not receive a score. */
export type Check = {
  /** File for per-item checks. */
  item?: string
  /** Configured metric name. */
  metric: string
  /** Confirmed violations take precedence over an incomplete run. */
  problems?: readonly string[] | undefined
  /** Why the absence of a violation does not establish a pass. */
  unknown?: string | undefined
}

/** Measures manifest conventions; Cargo metadata supplies the actual edition and dependency names after renaming and inheritance. */
export function* inspect(
  metadata: ReturnType<typeof Cargo.metadata>,
): Generator<Check> {
  const root = read('Cargo.toml')
  const workspace = table(root.workspace)
  const shared = table(workspace.dependencies)
  const declaredMocks = Object.entries(shared)
    .map((entry) => String(table(entry[1]).package ?? entry[0]))
    .filter((name) => mockCrates.has(name))
  const packages = metadata.packages
    .map((pkg) => ({ file: Cargo.relative(pkg.manifest_path), pkg }))
    .sort((a, b) => Number(a.file > b.file) - Number(a.file < b.file))
  for (const { file, pkg } of packages) {
    const manifest = read(file)
    const mocks = new Set([
      ...pkg.dependencies
        .map((dep) => dep.name)
        .filter((name) => mockCrates.has(name)),
      ...(file === 'Cargo.toml' ? declaredMocks : []),
    ])
    yield {
      item: file,
      metric: 'mock-crates',
      problems: [...mocks].map((name) => `depends on ${name}`),
    }
    yield {
      item: file,
      metric: 'edition',
      problems:
        pkg.edition === '2024' ? [] : [`edition is ${pkg.edition}, not 2024`],
    }
    if (!root.workspace) continue
    yield {
      item: file,
      metric: 'inherited-lints',
      problems:
        table(manifest.lints).workspace === true
          ? []
          : ['[lints] does not set `workspace = true`'],
    }
    const own = new Set<string>()
    for (const section of [
      manifest,
      ...Object.values(table(manifest.target)).map(table),
    ]) {
      for (const kind of [
        'dependencies',
        'build-dependencies',
        'dev-dependencies',
      ]) {
        for (const [name, value] of Object.entries(table(section[kind]))) {
          if (
            table(value).workspace !== true &&
            table(value).path === undefined
          )
            own.add(name)
        }
      }
    }
    yield {
      item: file,
      metric: 'shared-versions',
      problems: [...own].map(
        (name) => `${name} sets its own version, not \`workspace = true\``,
      ),
    }
  }
  if (!root.package)
    yield {
      item: 'Cargo.toml',
      metric: 'mock-crates',
      problems: declaredMocks.map((name) => `depends on ${name}`),
    }
  if (!root.workspace) {
    yield { metric: 'inherited-lints', unknown: 'not a workspace' }
    yield { metric: 'shared-versions', unknown: 'not a workspace' }
  }

  const filename = ['rust-toolchain', 'rust-toolchain.toml'].find((file) =>
    fs.existsSync(file),
  )
  const channel = (() => {
    if (!filename) return undefined
    const contents = fs.readFileSync(filename, 'utf8').trim()
    if (
      filename === 'rust-toolchain' &&
      !contents.includes('\n') &&
      !contents.startsWith('[')
    )
      return contents
    return table(table(Bun.TOML.parse(contents)).toolchain).channel
  })()
  yield {
    metric: 'toolchain',
    problems: (() => {
      if (!filename) return ['no rust-toolchain.toml']
      if (typeof channel === 'string' && /^\d+\.\d+(\.\d+)?$/.test(channel))
        return []
      return [`the channel is ${channel ?? 'unset'}, not a version`]
    })(),
  }
  yield {
    metric: 'rust-version',
    problems:
      (table(workspace.package)['rust-version'] ??
      table(root.package)['rust-version'])
        ? []
        : ['Cargo.toml declares no rust-version'],
  }

  const lints = table(root.workspace ? workspace.lints : root.lints)
  const clippy = lintTable(lints.clippy)
  const enabled = new Set(['warn', 'deny', 'forbid'])
  yield {
    metric: 'workspace-lints',
    problems: ['all', 'pedantic', ...restrictions]
      .filter((name) => !enabled.has(String(clippy.get(name))))
      .map((name) => `clippy::${name} is not enabled`),
  }
  yield {
    metric: 'allowed-lints',
    problems: [...clippy]
      .filter((entry) => entry[1] === 'allow' && !allowed.includes(entry[0]))
      .map((entry) => `clippy::${entry[0]} is allowed`),
  }
  const unsafe = lintTable(lints.rust).get('unsafe_code')
  yield {
    metric: 'unsafe-denied',
    problems:
      unsafe === 'deny' || unsafe === 'forbid'
        ? []
        : ['unsafe_code is not denied'],
  }

  const profile = table(table(root.profile).release)
  const effective = {
    ...profile,
    lto: profile.lto === true ? 'fat' : profile.lto,
    strip: profile.strip === true ? 'symbols' : profile.strip,
  } as Table
  const release = {
    'codegen-units': 1,
    lto: 'fat',
    'opt-level': 3,
    strip: 'symbols',
  }
  yield {
    metric: 'release-profile',
    problems: Object.entries(release)
      .filter((entry) => effective[entry[0]] !== entry[1])
      .map(
        (entry) =>
          `[profile.release] does not set ${entry[0]} = ${JSON.stringify(entry[1])}`,
      ),
  }
}
