/** @module Collects positive length diagnostics from optional linters. Silence is not evidence of file coverage. */
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

/** Confirmed length diagnostics, by tracked file and declaration line. */
export type Findings = Map<string, Map<number, string>>

/** One linter run: the scratch directory for its settings, the eligible files, and the length limit. */
type Run = {
  dir: string
  files: ReadonlySet<string>
  limit: number
}

function add(found: Findings, options: add.Options) {
  const rows = found.get(options.file) ?? new Map<number, string>()
  rows.set(options.line, options.reason)
  found.set(options.file, rows)
}

function clippy(found: Findings, options: Run) {
  const { dir, files, limit } = options
  if (
    ![...files].some((file) => file.endsWith('.rs')) ||
    !fs.existsSync('Cargo.toml') ||
    !Bun.which('cargo')
  )
    return
  const own = ['clippy.toml', '.clippy.toml'].find((file) =>
    fs.existsSync(file),
  )
  const settings = own
    ? fs.readFileSync(own, 'utf8').replace(/^too-many-lines-threshold.*$/m, '')
    : ''
  fs.writeFileSync(
    path.join(dir, 'clippy.toml'),
    `${settings}\ntoo-many-lines-threshold = ${limit}\n`,
  )

  const result = Bun.spawnSync(
    [
      'cargo',
      'clippy',
      '--workspace',
      '--all-targets',
      '--quiet',
      '--message-format=json',
      '--',
      '-A',
      'clippy::all',
      '-W',
      'clippy::too_many_lines',
    ],
    {
      env: { ...process.env, CLIPPY_CONF_DIR: dir },
      stderr: 'pipe',
    },
  )
  if (!result.success) {
    process.stderr.write(
      'Clippy did not finish. Measuring Rust length from source spans.\n',
    )
    return
  }

  for (const line of result.stdout.toString().split('\n')) {
    if (!line.startsWith('{')) continue
    type Span = { file_name: string; is_primary: boolean; line_start: number }
    const data = JSON.parse(line) as {
      manifest_path?: string
      message?: {
        code?: { code: string }
        message: string
        spans: readonly Span[]
      }
    }
    const msg = data.message
    if (msg?.code?.code !== 'clippy::too_many_lines') continue
    const span = msg.spans.find((entry) => entry.is_primary)
    if (!span) continue
    const paths = [
      process.cwd(),
      ...(data.manifest_path ? [path.dirname(data.manifest_path)] : []),
    ]
    // Cargo versions can report crate-relative or workspace-relative paths. Do not guess
    // between two tracked files. Both still have independent syntax measurements.
    const candidates = new Set(
      paths
        .map((dir) =>
          path.relative(process.cwd(), path.resolve(dir, span.file_name)),
        )
        .filter((file) => files.has(file)),
    )
    if (candidates.size === 1)
      for (const file of candidates)
        add(found, {
          file,
          line: span.line_start,
          reason: msg.message + ' (clippy::too_many_lines)',
        })
  }
}

function oxlint(found: Findings, options: Run) {
  const { dir, files, limit } = options
  const sources = [...files].filter((file) => /\.(?:[cm]?ts|tsx)$/.test(file))
  if (!sources.length) return
  const manifest = path.resolve('node_modules/oxlint/package.json')
  if (!fs.existsSync(manifest)) return
  const pkg = JSON.parse(fs.readFileSync(manifest, 'utf8')) as {
    bin: string | { oxlint: string }
  }
  const bin = path.resolve(
    path.dirname(manifest),
    typeof pkg.bin === 'string' ? pkg.bin : pkg.bin.oxlint,
  )
  const config = path.join(dir, 'oxlintrc.json')
  fs.writeFileSync(
    config,
    JSON.stringify({
      categories: { correctness: 'off' },
      rules: {
        'eslint/max-lines-per-function': [
          'error',
          { max: limit, skipBlankLines: false, skipComments: false },
        ],
      },
    }),
  )

  for (let i = 0; i < sources.length; i += 500) {
    const result = Bun.spawnSync([
      bin,
      '-c',
      config,
      '--format',
      'json',
      '--',
      ...sources.slice(i, i + 500),
    ])
    type Diagnostic = {
      code?: string
      filename: string
      labels: readonly { span: { line: number } }[]
      message: string
    }
    const diagnostics = (() => {
      try {
        return (
          JSON.parse(result.stdout.toString()) as {
            diagnostics?: readonly Diagnostic[]
          }
        ).diagnostics
      } catch {
        return undefined
      }
    })()
    if (!Array.isArray(diagnostics) || ![0, 1].includes(result.exitCode)) {
      process.stderr.write(
        'Oxlint did not return a report. Measuring TypeScript length from source spans.\n',
      )
      continue
    }
    for (const diag of diagnostics) {
      if (diag.code !== 'eslint(max-lines-per-function)') continue
      const file = path.relative(process.cwd(), path.resolve(diag.filename))
      const line = diag.labels[0]?.span.line
      if (files.has(file) && line)
        add(found, {
          file,
          line,
          reason: diag.message + ' (eslint/max-lines-per-function)',
        })
    }
  }
}

/** Removes repeated diagnostics for the same function from multiple Cargo targets. */
export function lengths(files: readonly string[], limit: number): Findings {
  const found: Findings = new Map()
  const eligible = new Set(files)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-lengths-'))

  try {
    clippy(found, { dir, files: eligible, limit })
    oxlint(found, { dir, files: eligible, limit })
    return found
  } finally {
    fs.rmSync(dir, { force: true, recursive: true })
  }
}

declare namespace add {
  /** The tracked file, the declaration line, and the linter's message. */
  type Options = {
    file: string
    line: number
    reason: string
  }
}
