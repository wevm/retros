/**
 * Measures a Cargo project against Rust conventions. Clippy and rustc report most of them from
 * one run with every lint the metrics read. The script reads the rest: module comments, unit tests
 * in `src/`, and the manifests as TOML. Each source file or manifest is an item, and a failing
 * check carries its first warning as the note.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

/** Lints that each count toward one metric, which clippy reports as warnings. */
const lints = {
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
} as const

/** Lints the conventions allow workspace-wide. */
const allowed = [
  'module_name_repetitions',
  'must_use_candidate',
  'similar_names',
  'too_many_lines',
]

/** Restriction lints the conventions turn on in `[workspace.lints.clippy]`. */
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

/** The release profile settings the conventions set. */
const release = {
  'codegen-units': 1,
  lto: 'fat',
  'opt-level': 3,
  strip: 'symbols',
} as const

/** Crates that replace real behavior in tests. */
const mockCrates = new Set([
  'faux',
  'mockall',
  'mockito',
  'mocktopus',
  'wiremock',
])

// `cargo metadata` lists the workspace's own packages, so nested projects outside it stay out.
const manifests = (
  JSON.parse(
    run(['cargo', 'metadata', '--no-deps', '--format-version', '1']) ||
      '{"packages":[]}',
  ).packages as { manifest_path: string }[]
)
  .map((entry) => path.relative(process.cwd(), entry.manifest_path))
  .sort()
const directories = manifests.map((file) => path.dirname(file))
const sources = list('*.rs').filter((file) =>
  directories.some(
    (directory) =>
      directory === '.' || directory === '' || file.startsWith(`${directory}/`),
  ),
)
const production = sources.filter(
  (file) => !/(^|\/)(tests|benches|examples)\//.test(file),
)
const lines: string[] = []
/** Clippy's warnings, once it ran. */
const cache: { clippy?: readonly Warning[] } = {}

if (!fs.existsSync('Cargo.toml')) {
  process.stderr.write('Found no Cargo.toml in the project root\n')
  process.exit(1)
}

style()
safety()
docs()
testing()
manifest()
slop()
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/**
 * Writes clippy's default and pedantic warnings, rustfmt's verdict, print macros, and unfinished
 * code.
 */
function style(): void {
  const warnings = clippy()
  const counted: ReadonlySet<string> = new Set(Object.values(lints).flat())
  for (const file of sources)
    check({
      item: file,
      metric: 'clippy',
      problems: warnings
        .filter(
          (warning) => warning.file === file && !counted.has(warning.code),
        )
        .map(describe),
    })

  // `cargo fmt --check -- -l` lists the absolute path of each file rustfmt would change.
  const unformatted = new Set(
    run(['cargo', 'fmt', '--all', '--check', '--', '-l'])
      .split('\n')
      .filter(Boolean)
      .map((file) => path.relative(process.cwd(), file)),
  )
  for (const file of sources)
    check({
      item: file,
      metric: 'formatted',
      problems: unformatted.has(file) ? ['rustfmt would change this file'] : [],
    })

  counts({
    files: production,
    metrics: ['print-macros', 'unfinished'],
    warnings,
  })
}

/**
 * Writes calls that can panic, lint suppressions, unsafe code, hash iteration, and shared
 * mutability.
 */
function safety(): void {
  const warnings = clippy()
  counts({
    files: production,
    metrics: [
      'panic-sites',
      'allow-attributes',
      'unsafe-code',
      'inline-always',
      'hash-iteration',
    ],
    warnings,
  })

  for (const file of production) {
    const text = fs.readFileSync(file, 'utf8')
    const graphs = [...text.matchAll(/Rc<\s*RefCell</g)].map(
      (match) => `line ${lineAt(text, match.index)}: Rc<RefCell<…>>`,
    )
    check({
      item: file,
      metric: 'shared-mutability',
      problems: [
        ...warnings
          .filter(
            (warning) =>
              warning.file === file &&
              (lints['shared-mutability'] as readonly string[]).includes(
                warning.code,
              ),
          )
          .map(describe),
        ...graphs,
      ],
    })
  }
}

/** Writes module comments, missing documentation, unreachable `pub` items, and rustdoc warnings. */
function docs(): void {
  for (const file of sources) {
    const first = fs
      .readFileSync(file, 'utf8')
      .split('\n')
      .find((line) => line.trim() !== '' && !line.startsWith('#!['))
    check({
      item: file,
      metric: 'module-docs',
      problems: first?.startsWith('//!')
        ? []
        : ['the file does not start with a `//!` comment'],
    })
  }

  const warnings = clippy()
  counts({
    files: production,
    metrics: ['missing-docs', 'unreachable-pub'],
    warnings,
  })

  const doc = Bun.spawnSync(
    ['cargo', 'doc', '--workspace', '--no-deps', '--quiet'],
    { env: { ...process.env, RUSTDOCFLAGS: '-D warnings' }, stderr: 'pipe' },
  )
  const error = /^(?:error|warning)(?:\[[^\]]+\])?: (.+)$/m.exec(
    doc.stderr.toString(),
  )
  check({
    metric: 'rustdoc',
    problems: doc.success
      ? []
      : [error?.[1] ?? `cargo doc exited with status ${doc.exitCode}`],
  })
}

/** Writes unit tests in `src/`, the share of inline snapshot assertions, and mocking crates. */
function testing(): void {
  for (const file of production) {
    const text = fs.readFileSync(file, 'utf8')
    const match = /#\[(cfg\(test\)|test)\]/.exec(text)
    check({
      item: file,
      metric: 'unit-tests',
      problems: match
        ? [`line ${lineAt(text, match.index)}: #[${match[1]}] outside tests/`]
        : [],
    })
  }

  const tests = sources.filter((file) => /(^|\/)tests\//.test(file))
  for (const file of tests) {
    const text = fs.readFileSync(file, 'utf8')
    const all = [
      ...text.matchAll(
        /\b(assert(_eq|_ne)?|debug_assert(_eq|_ne)?|(insta::)?assert_\w*snapshot)!\s*\(/g,
      ),
    ]
    const inline = all.filter(
      (match) =>
        /snapshot/.test(match[0]) &&
        /^[^;]*?,\s*@/.test(text.slice(match.index)),
    )
    check({
      item: file,
      metric: 'inline-snapshots',
      problems:
        inline.length < all.length
          ? [
              `${all.length - inline.length} of ${all.length} assertions are not inline snapshots`,
            ]
          : [],
    })
  }
  if (tests.length === 0)
    lines.push('inline-snapshots skip note "no files under tests/"')

  for (const file of new Set(['Cargo.toml', ...manifests])) {
    const toml = Bun.TOML.parse(fs.readFileSync(file, 'utf8')) as Toml
    const crates = Object.keys({
      ...toml['dev-dependencies'],
      ...toml.dependencies,
      ...toml.workspace?.dependencies,
    }).filter((name) => mockCrates.has(name))
    check({
      item: file,
      metric: 'mock-crates',
      problems: crates.map((name) => `depends on ${name}`),
    })
  }
}

/**
 * Writes the edition, toolchain, workspace lints, release profile, and dependencies of the
 * manifests.
 */
function manifest(): void {
  const root = Bun.TOML.parse(fs.readFileSync('Cargo.toml', 'utf8')) as Toml
  const workspace = root.workspace?.package ?? {}

  for (const file of manifests) {
    const toml = Bun.TOML.parse(fs.readFileSync(file, 'utf8')) as Toml
    if (!toml.package) continue
    const edition =
      typeof toml.package.edition === 'object'
        ? workspace.edition
        : toml.package.edition
    check({
      item: file,
      metric: 'edition',
      problems:
        edition === '2024'
          ? []
          : [`edition is ${edition ?? '2015, by default'}, not 2024`],
    })
    if (root.workspace) {
      check({
        item: file,
        metric: 'inherited-lints',
        problems:
          toml.lints?.workspace === true
            ? []
            : ['[lints] does not set `workspace = true`'],
      })
      const own = Object.entries<Toml | string>({
        ...toml.dependencies,
        ...toml['dev-dependencies'],
        ...toml['build-dependencies'],
      }).filter(
        (entry) =>
          !(
            typeof entry[1] === 'object' &&
            (entry[1].workspace === true || entry[1].path !== undefined)
          ),
      )
      check({
        item: file,
        metric: 'shared-versions',
        problems: own.map(
          (entry) =>
            `${entry[0]} sets its own version, not \`workspace = true\``,
        ),
      })
    }
  }
  if (!root.workspace) {
    lines.push('inherited-lints skip note "not a workspace"')
    lines.push('shared-versions skip note "not a workspace"')
  }

  const toolchain = ['rust-toolchain.toml', 'rust-toolchain'].find((file) =>
    fs.existsSync(file),
  )
  const channel =
    toolchain &&
    /channel\s*=\s*"([^"]+)"/.exec(fs.readFileSync(toolchain, 'utf8'))?.[1]
  check({
    metric: 'toolchain',
    problems: (() => {
      if (!toolchain) return ['no rust-toolchain.toml']
      if (/^\d+\.\d+(\.\d+)?$/.test(channel ?? '')) return []
      return [`the channel is ${channel ?? 'unset'}, not a version`]
    })(),
  })
  check({
    metric: 'rust-version',
    problems:
      (workspace['rust-version'] ?? root.package?.['rust-version'])
        ? []
        : ['Cargo.toml declares no rust-version'],
  })

  const table = root.workspace ? root.workspace.lints : root.lints
  const clippyLints = table?.clippy ?? {}
  const level = (name: string) => {
    const value = clippyLints[name]
    return typeof value === 'object' ? value.level : value
  }
  check({
    metric: 'workspace-lints',
    problems: [
      ...['all', 'pedantic']
        .filter((group) => level(group) !== 'warn' && level(group) !== 'deny')
        .map((group) => `clippy::${group} is not at warn`),
      ...restrictions
        .filter(
          (name) =>
            level(name) !== 'warn' &&
            level(name) !== 'deny' &&
            level(name) !== 'forbid',
        )
        .map((name) => `clippy::${name} is not enabled`),
    ],
  })
  check({
    metric: 'allowed-lints',
    problems: Object.entries<Toml | string>(clippyLints)
      .filter(
        (entry) =>
          (typeof entry[1] === 'object' ? entry[1].level : entry[1]) ===
            'allow' && !allowed.includes(entry[0]),
      )
      .map((entry) => `clippy::${entry[0]} is allowed`),
  })
  const rust = table?.rust ?? {}
  const unsafe =
    typeof rust.unsafe_code === 'object'
      ? rust.unsafe_code.level
      : rust.unsafe_code
  check({
    metric: 'unsafe-denied',
    problems:
      unsafe === 'deny' || unsafe === 'forbid'
        ? []
        : ['unsafe_code is not denied'],
  })

  const profile = root.profile?.release ?? {}
  check({
    metric: 'release-profile',
    problems: Object.entries(release)
      .filter((entry) => profile[entry[0]] !== entry[1])
      .map(
        (entry) =>
          `[profile.release] does not set ${entry[0]} = ${JSON.stringify(entry[1])}`,
      ),
  })

  // `cargo tree` prints each crate once with `--no-dedupe` off, so the lines past the workspace's
  // own are third-party crates.
  const tree = run([
    'cargo',
    'tree',
    '--workspace',
    '--edges',
    'normal',
    '--prefix',
    'none',
    '--format',
    '{p}',
  ])
    .split('\n')
    .filter(Boolean)
  const local = new Set(
    tree
      .filter((line) => line.includes('(/') || line.includes('(file://'))
      .map((line) => line.split(' ')[0]),
  )
  lines.push(
    `dependencies ${new Set(tree.map((line) => line.split(' ')[0]).filter((name) => !local.has(name))).size}`,
  )
}

/**
 * Writes the slapslop lints for generated Rust that need no type
 * information: clippy reports swallowed errors and redundant clones, and the script reads the rest
 * from the source. Each metric takes the name of the slapslop lint it follows.
 */
function slop(): void {
  const warnings = clippy()
  counts({
    files: production,
    metrics: ['swallowed-errors', 'redundant-clones'],
    warnings,
  })

  const patterns = {
    'comment-dividers': {
      files: sources,
      pattern: /^\s*\/\/+\s*([-=*#~_])\1{2,}/gm,
      text: 'a decorative divider',
    },
    'compound-asserts': {
      files: sources,
      pattern: /\b(?:debug_)?assert!\s*\((?:[^(),]|\([^()]*\))*?(?:&&|\|\|)/g,
      text: 'one assertion checks two conditions',
    },
    'duration-names': {
      files: sources,
      pattern:
        /Duration::from_(?:secs|millis|micros|nanos)\(\s*(?![a-z_]*(?:secs|seconds|ms|millis|us|micros|ns|nanos)\b)[a-z_][a-z0-9_]*\s*\)/g,
      text: 'a duration from a name without its unit',
    },
    'import-renames': {
      files: sources,
      pattern: /^\s*(?:pub\s+)?use\s[^;]*\bas\s+(?!_\b)\w+/gm,
      text: 'a renamed import',
    },
    'log-interpolation': {
      files: production,
      pattern:
        /\b(?:trace|debug|info|warn|error)!\s*\((?:[^")]*,\s*)?"[^"]*\{[^}]*\}/g,
      text: 'a log message that interpolates values, not fields',
    },
    'string-errors': {
      files: production,
      pattern:
        /Result<[^<>]*?,\s*(?:String|&'static str|&str|Box<dyn\s+(?:std::error::)?Error>)\s*>/g,
      text: 'a string as the error',
    },
    'test-sleeps': {
      files: sources.filter((file) => !production.includes(file)),
      pattern: /\b(?:thread|time)::sleep\s*\(/g,
      text: 'a test that sleeps a fixed time',
    },
    'unbounded-channels': {
      files: production,
      pattern:
        /\b(?:unbounded_channel|mpsc::channel|crossbeam_channel::unbounded|flume::unbounded)\s*(?:::<[^>]*>)?\s*\(\s*\)/g,
      text: 'a channel without a capacity',
    },
    'weak-error-assertions': {
      files: sources.filter((file) => !production.includes(file)),
      pattern: /\bassert!\s*\([^;]*\.is_err\(\)\s*\)/g,
      text: 'an assertion that only checks for an error',
    },
    'wide-tuples': {
      files: production,
      pattern:
        /(?:->|:|=)\s*\((?:[^(),]+|\([^()]*\)),(?:[^(),]+|\([^()]*\)),(?:[^()]+|\([^()]*\))\)/g,
      text: 'a tuple of three or more types',
    },
  }
  for (const [metric, rule] of Object.entries(patterns)) {
    if (rule.files.length === 0)
      lines.push(`${metric} skip note "no files to read"`)
    for (const file of rule.files) {
      const source = stripped(file)
      check({
        item: file,
        metric,
        problems: [...source.matchAll(rule.pattern)].map(
          (match) => `line ${lineAt(source, match.index)}: ${rule.text}`,
        ),
      })
    }
  }

  const reachable = modules()
  for (const file of sources)
    check({
      item: file,
      metric: 'orphaned-files',
      problems: reachable.has(file) ? [] : ['no module loads this file'],
    })
}

/**
 * Returns the source of a file with string literals blanked out, so patterns match only code and
 * comments.
 */
function stripped(file: string): string {
  return fs
    .readFileSync(file, 'utf8')
    .replace(
      /"(?:\\.|[^"\\])*"/g,
      (match) => `"${' '.repeat(match.length - 2)}"`,
    )
}

/** Returns the Rust files that a crate root reaches through `mod` declarations. */
function modules(): Set<string> {
  const roots = sources.filter((file) =>
    /(^|\/)(src\/(lib|main)\.rs|src\/bin\/[^/]+\.rs|src\/bin\/[^/]+\/main\.rs|(tests|benches|examples)\/[^/]+\.rs|(tests|benches|examples)\/[^/]+\/main\.rs|build\.rs)$/.test(
      file,
    ),
  )
  const reached = new Set<string>()
  const queue = [...roots]
  while (queue.length > 0) {
    const file = queue.pop()!
    if (reached.has(file) || !fs.existsSync(file)) continue
    reached.add(file)
    const directory =
      /(^|\/)(lib|main|mod)\.rs$/.test(file) || roots.includes(file)
        ? path.dirname(file)
        : file.replace(/\.rs$/, '')
    for (const match of fs
      .readFileSync(file, 'utf8')
      .matchAll(
        /(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;/g,
      ))
      queue.push(
        ...(match[1]
          ? [path.normalize(path.join(path.dirname(file), match[1]))]
          : [
              path.join(directory, `${match[2]}.rs`),
              path.join(directory, match[2]!, 'mod.rs'),
            ]),
      )
  }
  return reached
}

/** One warning from clippy or rustc. */
type Warning = { code: string; file: string; line: number; text: string }

/** A parsed Cargo.toml. */
// oxlint-disable-next-line typescript/no-explicit-any -- Cargo.toml tables have many shapes.
type Toml = Record<string, any>

/** Runs clippy once with every lint the categories read, and returns its warnings. */
function clippy(): readonly Warning[] {
  cache.clippy ??= runClippy()
  return cache.clippy
}

function runClippy(): readonly Warning[] {
  const flags = [
    '-W',
    'clippy::all',
    '-W',
    'clippy::pedantic',
    ...Object.values(lints)
      .flat()
      .flatMap((lint) => ['-W', lint]),
    ...allowed.flatMap((lint) => ['-A', `clippy::${lint}`]),
  ]
  const output = run([
    'cargo',
    'clippy',
    '--workspace',
    '--all-targets',
    '--quiet',
    '--message-format=json',
    '--',
    ...flags,
  ])
  return output
    .split('\n')
    .filter((line) => line.startsWith('{'))
    .map((line) => JSON.parse(line))
    .filter(
      (message) =>
        message.reason === 'compiler-message' &&
        message.message.level === 'warning',
    )
    .flatMap((message) => {
      const span = message.message.spans.find(
        (span: { is_primary: boolean }) => span.is_primary,
      )
      return span
        ? [
            {
              code: message.message.code?.code ?? '',
              file: path.normalize(span.file_name),
              line: span.line_start,
              text: message.message.message,
            },
          ]
        : []
    })
}

/** Writes one check per file for each metric, from the warnings of that metric's lints. */
function counts(options: counts.Options): void {
  const { files, metrics, warnings } = options
  for (const metric of metrics) {
    const codes: readonly string[] = lints[metric]
    for (const file of files)
      check({
        item: file,
        metric,
        problems: warnings
          .filter(
            (warning) => warning.file === file && codes.includes(warning.code),
          )
          .map(describe),
      })
  }
}

function describe(warning: Warning): string {
  return `line ${warning.line}: ${warning.text}${warning.code ? ` (${warning.code})` : ''}`
}

/** Returns the tracked and untracked files that match `pattern` and that Git does not ignore. */
function list(pattern: string): string[] {
  const result = Bun.spawnSync(
    [
      'git',
      'ls-files',
      '--cached',
      '--others',
      '--exclude-standard',
      '-z',
      '--',
      pattern,
      `**/${pattern}`,
    ],
    { stderr: 'ignore' },
  )
  const found = result.success
    ? result.stdout.toString().split('\0')
    : [...new Bun.Glob(`**/${pattern}`).scanSync()]
  return [...new Set(found)]
    .filter(
      (file) => file && fs.existsSync(file) && !/(^|\/)target\//.test(file),
    )
    .sort()
}

function lineAt(text: string, offset: number): number {
  return text.slice(0, offset).split('\n').length
}

/** Runs a command and returns its standard output. A failing exit status is a measurement. */
function run(command: string[]): string {
  return Bun.spawnSync(command, { stderr: 'ignore' }).stdout.toString()
}

/**
 * Writes a check for one item: it passes without problems, and fails with the first as its note.
 */
function check(options: check.Options): void {
  const { item, metric, problems } = options
  const subject = item === undefined ? metric : `${metric} ${quote(item)}`
  const more = problems.length > 1 ? ` (and ${problems.length - 1} more)` : ''
  lines.push(
    problems.length === 0
      ? `${subject} pass`
      : `${subject} fail${note(`${problems[0]}${more}`)}`,
  )
}

/** Quotes an item or note for a metric line, escaping `"` and `\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"').replaceAll('\n', ' ')}"`
}

/** Returns the end of a metric line that says why, such as ` note "line 8: …"`, or nothing. */
function note(text: string | undefined): string {
  return text ? ` note ${quote(text)}` : ''
}

declare namespace check {
  /** One check: the metric, its item, and the problems that fail it. */
  type Options = {
    item?: string | undefined
    metric: string
    problems: readonly string[]
  }
}

declare namespace counts {
  /** The warnings of one clippy run, the metrics to write, and the files that are their items. */
  type Options = {
    files: readonly string[]
    metrics: readonly (keyof typeof lints)[]
    warnings: readonly Warning[]
  }
}
