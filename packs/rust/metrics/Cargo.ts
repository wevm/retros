/** @module Reads workspace membership, compiler artifacts, and diagnostics from Cargo. */
import * as fs from 'node:fs'
import * as path from 'node:path'

type Target = { kind: string[]; src_path: string }
type Package = {
  dependencies: { name: string }[]
  edition: string
  id: string
  manifest_path: string
  name: string
  targets: Target[]
}
type Metadata = {
  packages: Package[]
  target_directory: string
  workspace_members: string[]
}
type Artifact = {
  filenames: string[]
  package_id: string
  reason: 'compiler-artifact'
  target: Target
}
type Diagnostic = {
  message: {
    code: { code: string } | null
    level: string
    message: string
    spans: {
      column_start: number
      file_name: string
      is_primary: boolean
      line_start: number
    }[]
  }
  package_id: string
  reason: 'compiler-message'
}

/** Runs Cargo without dropping stderr or treating a failed command as empty output. */
export function run(args: string[], env = process.env) {
  const result = Bun.spawnSync(['cargo', ...args], {
    env,
    stderr: 'pipe',
    stdout: 'pipe',
  })
  const stderr = result.stderr.toString()
  const error = stderr.split('\n').find((line) => /^error\b/.test(line))
  return {
    failure: result.success
      ? undefined
      : (error ?? `cargo ${args[0]} exited with status ${result.exitCode}`),
    stderr,
    stdout: result.stdout.toString(),
  }
}

/** Reads required metadata. No membership, source, or count can be inferred if this fails. */
export function metadata(): Metadata {
  const result = run(['metadata', '--no-deps', '--format-version', '1'])
  if (result.failure)
    throw new Error(`cargo metadata: ${result.stderr.trim() || result.failure}`)
  return JSON.parse(result.stdout) as Metadata
}

function messages(output: ReturnType<typeof run>) {
  const records = output.stdout
    .split('\n')
    .filter((line) => line.startsWith('{'))
    .map((line) => JSON.parse(line))
  const finished = records.some(
    (entry) => entry.reason === 'build-finished' && entry.success === true,
  )
  const entries: (Artifact | Diagnostic)[] = records.filter(
    (entry) =>
      entry.reason === 'compiler-artifact' ||
      entry.reason === 'compiler-message',
  )
  return {
    entries,
    failure:
      output.failure ??
      (finished
        ? undefined
        : 'Cargo did not report a successful build-finished message'),
  }
}

/** Counts distinct nonmember package IDs from a normal workspace build, including build dependencies. */
export function dependencies(members: ReadonlySet<string>) {
  const result = messages(
    run(['build', '--workspace', '--message-format=json']),
  )
  const ids = new Set(
    result.entries
      .filter(
        (entry) =>
          entry.reason === 'compiler-artifact' &&
          !members.has(entry.package_id),
      )
      .map((entry) => entry.package_id),
  )
  return { count: ids.size, failure: result.failure }
}

/** Converts an absolute or workspace-relative Cargo filename to a project item. */
export function relative(file: string): string {
  return path.relative(process.cwd(), path.resolve(file))
}

/** Returns the normal build artifacts' rustc dep-info, never stale artifacts from a directory scan. */
function inputs(artifact: Artifact): string[] {
  const files = new Set([relative(artifact.target.src_path)])
  for (const filename of artifact.filenames) {
    const directory = path.dirname(filename)
    const stem = path.parse(filename).name
    const hash = /-([0-9a-f]+)$/.exec(path.basename(directory))?.[1]
    const stems = [stem, stem.replace(/^lib/, '')]
    if (hash && artifact.target.kind.includes('custom-build'))
      stems.push(`build_script_build-${hash}`)
    for (const name of stems) {
      const depInfo = path.join(directory, `${name}.d`)
      if (!fs.existsSync(depInfo)) continue
      // rustc writes a separate empty Make rule for every input. This avoids splitting paths on spaces.
      for (const line of fs.readFileSync(depInfo, 'utf8').split('\n')) {
        if (line.endsWith('.rs:'))
          files.add(relative(line.slice(0, -1).replaceAll('\\ ', ' ')))
      }
    }
  }
  return [...files]
}

/** Reads Clippy's measured files, distinct lint locations, and any incomplete-build reason. */
export function clippy(options: {
  flags: string[]
  members: ReadonlySet<string>
}) {
  const result = messages(
    run([
      'clippy',
      '--workspace',
      '--all-targets',
      '--quiet',
      '--message-format=json',
      '--',
      ...options.flags,
    ]),
  )
  const files = new Map<string, Set<string>>()
  const diagnostics = new Map<
    string,
    { code: string; file: string; text: string }
  >()
  let compilerError: string | undefined
  for (const entry of result.entries) {
    if (!options.members.has(entry.package_id)) continue
    if (entry.reason === 'compiler-artifact') {
      for (const file of inputs(entry)) {
        const kinds = files.get(file) ?? new Set<string>()
        for (const kind of entry.target.kind) kinds.add(kind)
        files.set(file, kinds)
      }
      continue
    }
    const diagnostic = entry.message
    if (diagnostic.level !== 'error' && diagnostic.level !== 'warning') continue
    const code = diagnostic.code?.code ?? ''
    if (diagnostic.level === 'error') compilerError ??= diagnostic.message
    // An invalid program did not complete linting. A denied lint still identifies a real violation.
    if (diagnostic.level === 'error' && (!code || /^E\d+$/.test(code))) continue
    for (const span of diagnostic.spans.filter((span) => span.is_primary)) {
      const file = relative(span.file_name)
      const text = `line ${span.line_start}: ${diagnostic.message}${code ? ` (${code})` : ''}`
      diagnostics.set(JSON.stringify([file, span.column_start, text]), {
        code,
        file,
        text,
      })
    }
  }
  const failure =
    result.failure &&
    `cargo clippy did not complete: ${compilerError ?? result.failure}`
  return { diagnostics: [...diagnostics.values()], failure, files }
}

/** Reads rustfmt's visited paths as well as its mismatches; an empty diff alone does not prove a pass. */
export function formatted(metadata: Metadata) {
  // `--all` also formats local path dependencies that are not workspace members.
  const packages = metadata.packages.flatMap((pkg) => ['-p', pkg.name])
  const result = run(['fmt', ...packages, '--check', '--', '-l', '-v'])
  const changed = new Set<string>()
  const visited = new Set<string>()
  for (const line of `${result.stdout}\n${result.stderr}`.split('\n')) {
    if (line.startsWith('Formatting '))
      visited.add(relative(line.slice('Formatting '.length)))
    else if (path.isAbsolute(line) && line.endsWith('.rs'))
      changed.add(relative(line))
  }
  const failure =
    result.failure && (changed.size === 0 || /^error\b/m.test(result.stderr))
      ? result.failure
      : undefined
  return { changed, failure, visited }
}

/** Selects Git-visible project files owned by a member, plus files Cargo actually reached across package directories. */
export function sources(options: {
  metadata: Metadata
  reached: ReadonlySet<string>
}): string[] {
  const manifests = new Set(
    options.metadata.packages.map((entry) => relative(entry.manifest_path)),
  )
  const target = relative(options.metadata.target_directory)
  const listing = Bun.spawnSync(
    [
      'git',
      'ls-files',
      '--cached',
      '--others',
      '--exclude-standard',
      '-z',
      '--',
      '*.rs',
    ],
    { stderr: 'pipe' },
  )
  const files = listing.success
    ? listing.stdout.toString().split('\0')
    : [...new Bun.Glob('**/*.rs').scanSync()]
  // An ignored file can still be an explicit Cargo input. Never import external or generated trees.
  return [...new Set([...files, ...options.reached])]
    .filter((file) => {
      if (
        !file.endsWith('.rs') ||
        file.startsWith(`..${path.sep}`) ||
        path.isAbsolute(file) ||
        !fs.existsSync(file) ||
        file.startsWith(`${target}/`) ||
        /(?:^|\/)(?:\.git|node_modules|target)\//.test(file)
      )
        return false
      if (options.reached.has(file)) return true
      let directory = path.dirname(file)
      for (;;) {
        const manifest = path.join(directory, 'Cargo.toml')
        if (fs.existsSync(manifest)) return manifests.has(manifest)
        if (directory === '.') return false
        directory = path.dirname(directory)
      }
    })
    .sort()
}
