/** @module Measures TigerStyle in Git-tracked source and manifests. Function spans also report partial length improvements. */
import * as fs from 'node:fs'
import * as path from 'node:path'
import * as Linters from './linters.js'
import * as Manifests from './manifests.js'
import * as Syntax from './syntax.js'

const functionLimit = 70
const ignored =
  /(^|\/)(\.agents\/uphill\/packs|dist|build|node_modules|target|vendor|\.?zig-cache|zig-out)\/|\.d\.[cm]?ts$/
const tests =
  /(^|\/)(tests?|__tests__|benches)\/|[._](test|spec)\.\w+$|_test\.go$/i

const git = Bun.spawnSync([
  'git',
  'ls-files',
  '--cached',
  '--deduplicate',
  '-z',
])
if (!git.success) throw new Error(`Could not list tracked files: ${git.stderr}`)
const tracked = git.stdout
  .toString()
  .split('\0')
  .filter(
    (file) =>
      file &&
      !ignored.test(file) &&
      fs.lstatSync(file, { throwIfNoEntry: false })?.isFile(),
  )
  .sort()
const files = tracked.filter((file) =>
  Object.hasOwn(Syntax.extensions, path.extname(file)),
)
const manifests = tracked.filter((file) =>
  Manifests.names.includes(path.basename(file)),
)
const lines: string[] = []
const emitted = new Set<string>()
const lint = Linters.lengths(files, functionLimit)
const read = await Syntax.create()

function quote(value: string) {
  return `"${value
    .replaceAll('\\', '\\\\')
    .replaceAll('"', '\\"')
    .replace(/[\r\n]/g, ' ')}"`
}

function emit(metric: string, file: string, value: string, note?: string) {
  emitted.add(metric)
  lines.push(
    `${metric}${file ? ` ${quote(file)}` : ''} ${value}${note ? ` note ${quote(note)}` : ''}`,
  )
}

function check(
  metric: string,
  file: string,
  problems: readonly string[],
  unavailable?: string,
) {
  if (unavailable) {
    emit(metric, file, 'skip', unavailable)
    return
  }
  const more = problems.length > 5 ? ` (and ${problems.length - 5} more)` : ''
  emit(
    metric,
    file,
    problems.length ? 'fail' : 'pass',
    problems.slice(0, 5).join('; ') + more,
  )
}

function sentences(comments: ReturnType<typeof Syntax.measure>['comments']) {
  const paragraphs: typeof comments = []
  let lastLine = 0
  for (const comment of comments) {
    const last = paragraphs.at(-1)
    if (last && lastLine + 1 === comment.line && comment.text)
      last.text += ` ${comment.text}`
    else paragraphs.push({ ...comment })
    lastLine = comment.line
  }

  return paragraphs
    .filter(
      (comment) =>
        comment.text &&
        !/^(?:SAFETY|TODO|FIXME|NOTE|tidy|eslint|oxlint|prettier|@ts-|#region|#endregion|biome|clippy|cspell|istanbul|c8|https?:\/\/\S+$)/i.test(
          comment.text,
        ),
    )
    .filter(
      (comment) =>
        /^\p{Ll}/u.test(comment.text) || !/[.:!?)`]$/.test(comment.text),
    )
    .map((comment) => `line ${comment.line}: ${comment.text.slice(0, 50)}`)
}

for (const file of files) {
  const text = fs.readFileSync(file, 'utf8')
  const grammar =
    Syntax.extensions[path.extname(file) as keyof typeof Syntax.extensions]
  const source = await read(grammar, text, (root) =>
    Syntax.measure(root, grammar, text),
  )
  const { error, functions } = source
  check('source-coverage', file, error ? [error] : [])

  const lengths: string[] = []
  const confirmed = new Map(lint.get(file))
  const occurrences = new Map<string, number>()
  const starts = new Map<number, number>()
  for (const fn of functions)
    starts.set(fn.line, (starts.get(fn.line) ?? 0) + 1)
  if (!error)
    for (const fn of functions) {
      const n = (occurrences.get(fn.name) ?? 0) + 1
      occurrences.set(fn.name, n)
      const size = fn.end - fn.line + 1
      emit(
        'function-lines',
        `${file}::${fn.name}${n > 1 ? `#${n}` : ''}`,
        String(size),
        `lines ${fn.line}-${fn.end}`,
      )
      const diagnostic =
        starts.get(fn.line) === 1 ? confirmed.get(fn.line) : undefined
      if (size > functionLimit || diagnostic)
        lengths.push(
          `line ${fn.line}: ${fn.name} has ${size} lines${diagnostic ? `; ${diagnostic}` : ''}`,
        )
      if (diagnostic) confirmed.delete(fn.line)
    }
  for (const [line, message] of confirmed)
    lengths.push(`line ${line}: ${message}`)
  check('function-length', file, lengths, lengths.length ? undefined : error)
  check(
    'recursion',
    file,
    functions.flatMap((fn) =>
      fn.calls.map((line) => `line ${line}: ${fn.name} calls itself`),
    ),
    error,
  )
  if (grammar === 'rust' || grammar === 'zig')
    check('sized-integers', file, source.sized, error)

  if (!tests.test(file)) {
    const production = functions.filter((fn) => fn.production)
    if (!error && production.length) {
      const total = production.reduce((sum, fn) => sum + fn.assertions, 0)
      const density = total / production.length
      const lowest = [...production]
        .sort((a, b) => a.assertions - b.assertions)
        .slice(0, 3)
      const detail = lowest
        .map((fn) => `${fn.name} (${fn.assertions}, line ${fn.line})`)
        .join(', ')
      emit(
        'assertion-density',
        file,
        // Keep small changes visible near the target while retaining the usual two-decimal display.
        Number.isInteger(density * 100) ? density.toFixed(2) : String(density),
        `${total} assertions in ${production.length} functions: ${detail}`,
      )
    }
    check('compound-assertions', file, source.compounds, error)
  }

  check(
    'line-length',
    file,
    text.split(/\r?\n/).flatMap((line, i) => {
      const columns = [...line.replaceAll('\t', '    ')].length
      return columns > 100 ? [`line ${i + 1}: ${columns} columns`] : []
    }),
  )
  check('comment-sentences', file, sentences(source.comments), error)
  check(
    'indentation',
    file,
    grammar === 'go'
      ? []
      : source.code.split('\n').flatMap((line, i) => {
          if (!line.trim()) return []
          const indent = /^[ \t]*/.exec(line)![0]
          if (indent.includes('\t')) return [`line ${i + 1}: a tab`]
          return indent.length % 4
            ? [`line ${i + 1}: ${indent.length} spaces`]
            : []
        }),
    error,
  )
  const name = path.basename(file, path.extname(file))
  check(
    'file-names',
    file,
    /^[a-z0-9_]+$/.test(name)
      ? []
      : [`${path.basename(file)} is not snake_case`],
    path.basename(file) === 'Package.swift'
      ? 'SwiftPM requires this manifest name'
      : undefined,
  )
}

for (const file of manifests) {
  const names = await Manifests.dependencies(
    path.basename(file),
    fs.readFileSync(file, 'utf8'),
    read,
  )
  check(
    'zero-dependencies',
    file,
    [...new Set(names)].map((name) => `depends on ${name}`),
    names ? undefined : 'could not read static package declarations',
  )
}

// Counts with a numeric target cannot mix with a `skip` check. Leave density unmeasured without a denominator.
if (!emitted.has('assertion-density'))
  process.stderr.write(
    'No named production functions could be measured for assertion-density.\n',
  )
for (const metric of [
  'comment-sentences',
  'compound-assertions',
  'file-names',
  'function-length',
  'function-lines',
  'indentation',
  'line-length',
  'recursion',
  'sized-integers',
  'source-coverage',
  'zero-dependencies',
]) {
  if (!emitted.has(metric))
    emit(metric, '', 'skip', 'no applicable files or functions')
}
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)
