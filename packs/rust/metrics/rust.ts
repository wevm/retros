/** @module Writes Cargo diagnostics, syntax-tree checks, and manifest settings as Rust pack metrics. */
import * as fs from 'node:fs'
import * as Cargo from './Cargo.js'
import * as Manifest from './Manifest.js'
import * as Source from './Source.js'

const lints: Record<string, readonly string[]> = {
  'allow-attributes': [
    'clippy::allow_attributes',
    'clippy::allow_attributes_without_reason',
  ],
  'hash-iteration': ['clippy::iter_over_hash_type'],
  'inline-always': ['clippy::inline_always'],
  'missing-docs': ['missing_docs'],
  'panic-sites': [
    'clippy::expect_used',
    'clippy::get_unwrap',
    'clippy::indexing_slicing',
    'clippy::panic',
    'clippy::unreachable',
    'clippy::unwrap_used',
  ],
  'print-macros': [
    'clippy::dbg_macro',
    'clippy::print_stderr',
    'clippy::print_stdout',
  ],
  'redundant-clones': ['clippy::redundant_clone'],
  'shared-mutability': ['clippy::rc_buffer', 'clippy::rc_mutex'],
  'swallowed-errors': ['clippy::map_err_ignore'],
  unfinished: ['clippy::todo', 'clippy::unimplemented'],
  'unreachable-pub': ['unreachable_pub'],
  'unsafe-code': ['clippy::undocumented_unsafe_blocks', 'unsafe_code'],
}

function quote(text: string): string {
  return `"${text
    .replaceAll('\\', '\\\\')
    .replaceAll('"', '\\"')
    .replace(/[\r\n]/g, ' ')}"`
}

function line(check: Manifest.Check): string {
  const subject = check.item
    ? `${check.metric} ${quote(check.item)}`
    : check.metric
  const problems = check.problems ?? []
  if (problems.length) {
    const more = problems.length > 1 ? ` (and ${problems.length - 1} more)` : ''
    return `${subject} fail note ${quote(`${problems[0]}${more}`)}`
  }
  if (check.unknown) return `${subject} skip note ${quote(check.unknown)}`
  return `${subject} pass`
}

async function measure(): Promise<readonly string[]> {
  if (!fs.existsSync('Cargo.toml'))
    throw new Error('Found no Cargo.toml in the project root')
  const metadata = Cargo.metadata()
  const members = new Set(metadata.workspace_members)
  const counted = new Set(Object.values(lints).flat())
  const clippy = Cargo.clippy({
    flags: [
      ...['clippy::all', 'clippy::pedantic', ...counted].flatMap((lint) => [
        '-W',
        lint,
      ]),
      ...Manifest.allowed.flatMap((lint) => ['-A', `clippy::${lint}`]),
    ],
    members,
  })
  const formatted = Cargo.formatted(metadata)
  const targets = metadata.packages.flatMap((pkg) => pkg.targets)
  const reached = new Set([
    ...targets.map((target) => Cargo.relative(target.src_path)),
    ...formatted.visited,
    ...clippy.files.keys(),
    ...clippy.diagnostics.map((d) => d.file),
  ])
  const sources = Cargo.sources({ metadata, reached })
  const parser = await Source.parser()
  const parsed = new Map(
    sources.map((file) => [
      file,
      Source.inspect(parser, fs.readFileSync(file, 'utf8')),
    ]),
  )
  parser.delete()

  const testKinds = new Set(['bench', 'example', 'test'])
  const roles = (file: string) =>
    clippy.files.get(file) ??
    new Set(
      targets
        .filter((target) => Cargo.relative(target.src_path) === file)
        .flatMap((target) => target.kind),
    )
  const production = sources.filter((file) => {
    const kinds = roles(file)
    return kinds.size
      ? [...kinds].some((kind) => !testKinds.has(kind))
      : !/(^|\/)(tests|benches|examples)\//.test(file)
  })
  const tests = sources.filter(
    (file) => roles(file).has('test') || /(^|\/)tests\//.test(file),
  )
  const checks: Manifest.Check[] = []
  const syntax: Record<string, readonly string[]> = {
    'comment-dividers': sources,
    'compound-asserts': sources,
    'duration-names': sources,
    'import-renames': sources,
    'inline-snapshots': tests,
    'log-interpolation': production,
    'module-docs': sources,
    'string-errors': production,
    'test-sleeps': tests,
    'unbounded-channels': production,
    'unit-tests': production,
    'weak-error-assertions': tests,
    'wide-tuples': production,
  }
  for (const [metric, files] of Object.entries(syntax)) {
    for (const item of files) {
      const source = parsed.get(item)!
      checks.push({
        item,
        metric,
        problems: source.problems.get(metric),
        unknown: source.failure,
      })
    }
    if (!files.length)
      checks.push({
        item: 'Rust workspace',
        metric,
        unknown: 'no applicable Rust source files',
      })
  }

  const groups: Record<string, readonly string[]> = { clippy: [], ...lints }
  for (const [metric, codes] of Object.entries(groups)) {
    const files = metric === 'clippy' ? sources : production
    for (const item of files) {
      const problems = clippy.diagnostics
        .filter(
          (d) =>
            d.file === item &&
            (metric === 'clippy'
              ? !counted.has(d.code)
              : codes.includes(d.code)),
        )
        .map((d) => d.text)
      const source =
        metric === 'shared-mutability' ? parsed.get(item) : undefined
      problems.push(...(source?.problems.get(metric) ?? []))
      checks.push({
        item,
        metric,
        problems,
        unknown:
          source?.failure ??
          clippy.failure ??
          (clippy.files.has(item)
            ? undefined
            : 'cargo clippy did not compile this file'),
      })
    }
    if (!files.length)
      checks.push({ metric, unknown: 'no applicable Rust source files' })
  }

  for (const item of sources) {
    const unknown = clippy.failure ?? formatted.failure
    checks.push({
      item,
      metric: 'orphaned-files',
      problems:
        !reached.has(item) && !unknown
          ? ['no workspace target loads this file']
          : [],
      unknown: reached.has(item) ? undefined : unknown,
    })
    checks.push({
      item,
      metric: 'formatted',
      problems: formatted.changed.has(item)
        ? ['rustfmt would change this file']
        : [],
      unknown:
        formatted.failure ??
        (formatted.visited.has(item)
          ? undefined
          : 'rustfmt did not visit this file'),
    })
  }
  if (!sources.length) {
    checks.push(
      { metric: 'orphaned-files', unknown: 'no Rust source files' },
      {
        item: 'Rust workspace',
        metric: 'formatted',
        unknown: 'no Rust source files',
      },
    )
  }

  const doc = Cargo.run(['doc', '--workspace', '--no-deps', '--quiet'], {
    ...process.env,
    RUSTDOCFLAGS: `${process.env.RUSTDOCFLAGS ?? ''} -D warnings`.trim(),
  })
  checks.push({
    metric: 'rustdoc',
    problems: doc.failure
      ? [doc.failure.replace(/^error(?:\[[^\]]+\])?: /, '')]
      : [],
  })
  checks.push(...Manifest.inspect(metadata))
  const deps = Cargo.dependencies(members)
  return [
    ...checks.map(line),
    deps.failure
      ? line({ metric: 'dependencies', unknown: deps.failure })
      : `dependencies ${deps.count}`,
  ]
}

try {
  const lines = await measure()
  fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)
} catch (error) {
  process.stderr.write(
    `${error instanceof Error ? error.message : String(error)}\n`,
  )
  process.exitCode = 1
}
