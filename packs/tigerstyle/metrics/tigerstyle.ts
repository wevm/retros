/**
 * Measures source files against TigerStyle (https://tigerstyle.dev), the TigerBeetle coding style:
 * a hard limit of 70 lines per function and 100 columns per line, two assertions per function on
 * average, no recursion, split compound assertions, comments written as sentences, 4-space
 * indentation, `snake_case` file names, explicitly sized integers, and zero dependencies. Clippy's
 * `too_many_lines` and oxlint's `max-lines-per-function` measure function length in Rust and
 * TypeScript. No linter reads the other rules, so a small scanner removes comments and strings and
 * matches braces, and the same checks read Rust, Zig, Go, Swift, C, and TypeScript. Each file is an
 * item.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

/** File extensions the scanner reads, with the syntax of each language. */
const languages: Record<string, Language> = {
  '.c': {
    assertion: /\bassert\s*\(/g,
    functions: /^[\w*\s]+?\b(\w+)\s*\([^;{}]*\)\s*\{/gm,
  },
  '.go': {
    assertion: /\bassert\w*\s*\(/g,
    functions: /\bfunc\s+(?:\([^)]*\)\s*)?(\w+)\s*[[(]/g,
  },
  '.h': {
    assertion: /\bassert\s*\(/g,
    functions: /^[\w*\s]+?\b(\w+)\s*\([^;{}]*\)\s*\{/gm,
  },
  '.rs': {
    assertion: /\b(?:debug_)?assert(?:_eq|_ne)?!\s*\(/g,
    functions: /\bfn\s+(\w+)/g,
  },
  '.swift': {
    assertion: /\b(?:assert|precondition)\s*\(/g,
    functions: /\bfunc\s+(\w+)/g,
  },
  '.ts': {
    assertion: /\b(?:assert(?:\.\w+)?|console\.assert|invariant)\s*\(/g,
    functions:
      /\bfunction\s*\*?\s*(\w+)|\b(?:const|let)\s+(\w+)\s*=\s*(?:async\s*)?(?:\([^()]*\)|\w+)\s*(?::[^=]+)?=>\s*\{|^\s*(?:(?:public|private|protected|static|async|override|readonly)\s+)*(?!(?:if|for|while|switch|catch|with|return|function)\b)(\w+)\s*\([^()]*\)\s*(?::\s*[^{;]+)?\{/gm,
  },
  '.zig': {
    assertion: /\b(?:std\.debug\.)?assert\s*\(/g,
    functions: /\bfn\s+(\w+)\s*\(/g,
  },
}
languages['.tsx'] = languages['.ts']!
languages['.mts'] = languages['.ts']!

/**
 * Paths that are not maintained source: tests, generated code, installed packages, and vendored
 * packs.
 */
const ignored = [
  /(^|\/)(\.agents\/packs|dist|build|node_modules|target|vendor|zig-cache|\.zig-cache|zig-out)\//,
  /\.d\.ts$/,
]
const tests =
  /(^|\/)(tests?|__tests__|benches)\/|[._](test|spec)\.\w+$|_test\.go$/

/** Words that open a block like a function header does, but are not functions. */
const keywords = new Set([
  'catch',
  'else',
  'for',
  'if',
  'return',
  'sizeof',
  'switch',
  'while',
])

const functionLines = 70
const lineColumns = 100

const files = list(
  Object.keys(languages).map((extension) => `*${extension}`),
).filter((file) => !ignored.some((pattern) => pattern.test(file)))
const lines: string[] = []
/** Parsed files and the line starts of each text, computed once. */
const lineStarts = new Map<string, number[]>()
const scanned = new Map<string, ReturnType<typeof parse>>()

if (files.length === 0) {
  process.stderr.write(
    `Found no source files with the extensions ${Object.keys(languages).join(', ')}\n`,
  )
  process.exit(1)
}

limits()
assertions()
style()
dependencies()
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Writes function length, line length, recursion, and architecture-specific integers. */
function limits(): void {
  const linted = { ...clippyLengths(), ...oxlintLengths() }
  for (const file of files) {
    const source = scan(file)
    check({
      item: file,
      metric: 'function-length',
      problems:
        linted[file] ??
        source.functions
          .filter((fn) => fn.lines > functionLines)
          .map((fn) => `line ${fn.line}: ${fn.name} has ${fn.lines} lines`),
    })

    check({
      item: file,
      metric: 'line-length',
      problems: source.text.split('\n').flatMap((line, index) => {
        const columns = [...line.replaceAll('\t', '    ')].length
        return columns > lineColumns
          ? [`line ${index + 1}: ${columns} columns`]
          : []
      }),
    })

    check({
      item: file,
      metric: 'recursion',
      problems: source.functions
        .filter((fn) =>
          new RegExp(
            `(?<![\\w.:])(?:self\\.|this\\.|Self::)?${fn.name}\\s*\\(`,
          ).test(source.code.slice(fn.body, fn.end)),
        )
        .map((fn) => `line ${fn.line}: ${fn.name} calls itself`),
    })
  }

  const sized = files.filter((file) => /\.(rs|zig)$/.test(file))
  for (const file of sized) {
    const source = scan(file)
    check({
      item: file,
      metric: 'sized-integers',
      problems: [...source.code.matchAll(/\b[ui]size\b/g)].map(
        (match) => `line ${lineAt(source.code, match.index)}: ${match[0]}`,
      ),
    })
  }
  if (sized.length === 0)
    lines.push('sized-integers skip note "no Rust or Zig files"')
}

/** Writes assertions per function and compound assertions. Tests are not counted. */
function assertions(): void {
  const production = files.filter((file) => !tests.test(file))
  for (const file of production) {
    const source = scan(file)
    const found = [...source.code.matchAll(source.language.assertion)]
    if (source.functions.length > 0)
      lines.push(
        `assertion-density ${quote(file)} ${(found.length / source.functions.length).toFixed(2)}${note(`${found.length} assertions in ${source.functions.length} functions`)}`,
      )

    check({
      item: file,
      metric: 'compound-assertions',
      problems: found.flatMap((match) => {
        const argument = first(source.code, match.index + match[0].length)
        return /&&|\|\||\band\b|\bor\b/.test(argument)
          ? [
              `line ${lineAt(source.code, match.index)}: ${match[0]}${argument.trim().slice(0, 40)}…)`,
            ]
          : []
      }),
    })
  }
  if (!production.some((file) => scan(file).functions.length > 0))
    process.stderr.write(
      'Found no functions outside tests, so assertion-density has no value\n',
    )
}

/** Writes comment sentences, indentation, and file names. */
function style(): void {
  for (const file of files) {
    const source = scan(file)
    check({
      item: file,
      metric: 'comment-sentences',
      problems: source.comments
        .filter(
          (comment) =>
            !/^(SAFETY|TODO|FIXME|NOTE|tidy|eslint|oxlint|prettier|@ts-|#region|#endregion|biome|clippy|cspell|istanbul|c8)/i.test(
              comment.text,
            ),
        )
        .filter(
          (comment) =>
            comment.text !== '' && !/^https?:\/\/\S+$/.test(comment.text),
        )
        .filter(
          (comment) =>
            !/^[^\p{Ll}]/u.test(comment.text) ||
            !/[.:!?)`]$/.test(comment.text),
        )
        .map(
          (comment) =>
            `line ${comment.line}: "${comment.text.length > 50 ? `${comment.text.slice(0, 47)}...` : comment.text}"`,
        ),
    })

    const unit = file.endsWith('.go') ? undefined : 4
    check({
      item: file,
      metric: 'indentation',
      problems:
        unit === undefined
          ? []
          : source.code.split('\n').flatMap((line, index) => {
              const indent = /^[ \t]*/.exec(line)![0]
              if (line.trim() === '') return []
              if (indent.includes('\t')) return [`line ${index + 1}: a tab`]
              return indent.length % unit === 0
                ? []
                : [`line ${index + 1}: ${indent.length} spaces`]
            }),
    })

    const name = path.basename(file).replace(/\.[^.]+$/, '')
    check({
      item: file,
      metric: 'file-names',
      problems: /^[a-z0-9_]+$/.test(name)
        ? []
        : [`${path.basename(file)} is not snake_case`],
    })
  }
}

/** Writes whether each manifest declares no runtime dependencies. */
function dependencies(): void {
  const manifests = list([
    'Cargo.toml',
    'package.json',
    'build.zig.zon',
    'go.mod',
  ]).filter((file) => !ignored.some((pattern) => pattern.test(file)))
  for (const file of manifests) {
    const text = fs.readFileSync(file, 'utf8')
    const names = (() => {
      if (file.endsWith('package.json'))
        return Object.keys(JSON.parse(text).dependencies ?? {})
      if (file.endsWith('Cargo.toml'))
        return Object.keys(
          (Bun.TOML.parse(text) as { dependencies?: object }).dependencies ??
            {},
        )
      if (file.endsWith('go.mod'))
        return [
          ...text.matchAll(
            /^\s*(?:require\s+)?([\w.-]+\.[\w./-]+)\s+v[\d.]+(?!.*\/\/ indirect)/gm,
          ),
        ].map((match) => match[1]!)
      const block =
        /\.dependencies\s*=\s*\.\{([\s\S]*?)\n\s*\},/.exec(text)?.[1] ?? ''
      return [...block.matchAll(/\.(@"[^"]+"|\w+)\s*=\s*\.\{/g)].map(
        (match) => match[1]!,
      )
    })()
    check({
      item: file,
      metric: 'zero-dependencies',
      problems: names.map((name) => `depends on ${name}`),
    })
  }
  if (manifests.length === 0)
    lines.push('zero-dependencies skip note "no manifest"')
}

/**
 * Returns clippy's `too_many_lines` warnings for each Rust file, with its threshold at the limit,
 * or nothing without a Cargo project or cargo. Clippy reads its threshold from `CLIPPY_CONF_DIR`,
 * so the script writes the project's own clippy.toml there with the threshold set.
 */
function clippyLengths(): Record<string, readonly string[]> {
  const rust = files.filter((file) => file.endsWith('.rs'))
  if (rust.length === 0 || !fs.existsSync('Cargo.toml') || !Bun.which('cargo'))
    return {}

  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-clippy-'))
  const own = ['clippy.toml', '.clippy.toml'].find((file) =>
    fs.existsSync(file),
  )
  const settings = own
    ? fs.readFileSync(own, 'utf8').replace(/^too-many-lines-threshold.*$/m, '')
    : ''
  fs.writeFileSync(
    path.join(directory, 'clippy.toml'),
    `${settings}\ntoo-many-lines-threshold = ${functionLines}\n`,
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
      env: { ...process.env, CLIPPY_CONF_DIR: directory },
      stderr: 'ignore',
    },
  )
  if (!result.success && result.stdout.length === 0) return {}

  const found: Record<string, string[]> = Object.fromEntries(
    rust.map((file) => [file, []]),
  )
  for (const line of result.stdout
    .toString()
    .split('\n')
    .filter((line) => line.startsWith('{'))) {
    const message = JSON.parse(line)
    if (message.message?.code?.code !== 'clippy::too_many_lines') continue
    const span = message.message.spans.find(
      (span: { is_primary: boolean }) => span.is_primary,
    )
    found[path.normalize(span.file_name)]?.push(
      `line ${span.line_start}: ${message.message.message} (clippy::too_many_lines)`,
    )
  }
  return found
}

/**
 * Returns oxlint's `max-lines-per-function` warnings for each TypeScript file, or nothing without
 * oxlint.
 */
function oxlintLengths(): Record<string, readonly string[]> {
  const typescript = files.filter((file) => /\.m?tsx?$/.test(file))
  const linter = bin('oxlint')
  if (typescript.length === 0 || !linter) return {}

  const config = path.join(
    fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-oxlint-')),
    'oxlintrc.json',
  )
  fs.writeFileSync(
    config,
    JSON.stringify({
      categories: {},
      rules: {
        'eslint/max-lines-per-function': [
          'error',
          { max: functionLines, skipBlankLines: false, skipComments: false },
        ],
      },
    }),
  )
  const found: Record<string, string[]> = Object.fromEntries(
    typescript.map((file) => [file, []]),
  )
  for (let start = 0; start < typescript.length; start += 500) {
    const output = Bun.spawnSync([
      linter,
      '-c',
      config,
      '--format',
      'json',
      ...typescript.slice(start, start + 500),
    ]).stdout.toString()
    const report = JSON.parse(output.slice(output.indexOf('{'))) as {
      diagnostics: {
        filename: string
        labels: { span: { line: number } }[]
        message: string
      }[]
    }
    for (const diagnostic of report.diagnostics)
      found[path.normalize(diagnostic.filename)]?.push(
        `line ${diagnostic.labels[0]?.span.line ?? 1}: ${diagnostic.message} (eslint/max-lines-per-function)`,
      )
  }
  return found
}

/** The syntax of one language: how it declares functions and asserts. */
type Language = { assertion: RegExp; functions: RegExp }

/** One function: its name, its first line, its length, and the offsets of its body in `code`. */
type Fn = {
  body: number
  end: number
  line: number
  lines: number
  name: string
}

function scan(file: string): ReturnType<typeof parse> {
  const cached = scanned.get(file)
  if (cached) return cached
  const result = parse(file)
  scanned.set(file, result)
  return result
}

/**
 * Reads a file, and returns its text, its code with comments and strings blanked out, its own-line
 * comments, and its functions. Blanking keeps every offset and newline, so lines and columns match
 * the file.
 */
function parse(file: string) {
  const text = fs.readFileSync(file, 'utf8')
  const extension = path.extname(file)
  const language = languages[extension]!
  const rust = extension === '.rs'
  const zig = extension === '.zig'
  const typescript = /^\.m?tsx?$/.test(extension)
  const code = text.split('')
  const comments: { line: number; text: string }[] = []

  const blank = (from: number, to: number) => {
    for (let index = from; index < to; index++)
      if (code[index] !== '\n') code[index] = ' '
  }

  let index = 0
  let previous = ''
  while (index < code.length) {
    const char = code[index]!
    const next = code[index + 1]

    if (char === '/' && next === '/') {
      const end =
        text.indexOf('\n', index) === -1
          ? text.length
          : text.indexOf('\n', index)
      const before = text.slice(text.lastIndexOf('\n', index - 1) + 1, index)
      // Doc comments (`///` and `//!`) are Markdown, with headings and lists, not sentences.
      if (
        before.trim() === '' &&
        !/^\/\/[/!]/.test(text.slice(index, index + 3))
      )
        comments.push({
          line: lineAt(text, index),
          text: text
            .slice(index, end)
            .replace(/^\/\/[/!]?\s?/, '')
            .trim(),
        })
      blank(index, end)
      index = end
      continue
    }
    if (char === '/' && next === '*') {
      const end =
        text.indexOf('*/', index + 2) === -1
          ? text.length
          : text.indexOf('*/', index + 2) + 2
      blank(index, end)
      index = end
      continue
    }
    if (zig && char === '\\' && next === '\\') {
      const end =
        text.indexOf('\n', index) === -1
          ? text.length
          : text.indexOf('\n', index)
      blank(index, end)
      index = end
      continue
    }
    if (
      rust &&
      char === 'r' &&
      /^r#*"/.test(text.slice(index, index + 8)) &&
      !/\w/.test(text[index - 1] ?? '')
    ) {
      const hashes = /^r(#*)"/.exec(text.slice(index))![1]!
      const end = text.indexOf(`"${hashes}`, index + hashes.length + 2)
      const stop = end === -1 ? text.length : end + hashes.length + 1
      blank(index, stop)
      index = stop
      continue
    }
    if (char === "'" && rust) {
      // A char literal closes within a few characters, and a lifetime does not close at all.
      const literal =
        /^'(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'])'/u.exec(
          text.slice(index),
        )
      if (literal) {
        blank(index, index + literal[0].length)
        index += literal[0].length
      } else index++
      continue
    }
    if (char === '"' || char === "'" || char === '`') {
      const end = closing(text, { quote: char, start: index })
      blank(index, end)
      index = end
      continue
    }
    if (
      typescript &&
      char === '/' &&
      /(^|[(,=:[!&|?{};]|\breturn)\s*$/.test(previous)
    ) {
      const match = /^\/(?:\\.|\[(?:\\.|[^\]])*\]|[^/\n\\])+\/[a-z]*/.exec(
        text.slice(index),
      )
      if (match) {
        blank(index, index + match[0].length)
        index += match[0].length
        continue
      }
    }
    if (char.trim() !== '') previous = (previous + char).slice(-8)
    index++
  }

  const blanked = code.join('')
  const functions: Fn[] = []
  for (const match of blanked.matchAll(language.functions)) {
    const name = match.slice(1).find(Boolean)
    if (!name || keywords.has(name)) continue
    const body = blanked.indexOf('{', match.index)
    const stop = blanked.indexOf(';', match.index)
    if (body === -1 || (stop !== -1 && stop < body && !match[0].includes('{')))
      continue
    const end = brace(blanked, body)
    const line = lineAt(
      blanked,
      match.index + match[0].length - match[0].trimStart().length,
    )
    functions.push({
      body: body + 1,
      end,
      line,
      lines: lineAt(blanked, end) - line + 1,
      name,
    })
  }

  return { code: blanked, comments: merge(comments), functions, language, text }
}

/**
 * Returns the offset after the quote that closes a string, skipping escapes and template
 * expressions.
 */
function closing(text: string, options: closing.Options): number {
  const { quote, start } = options
  let index = start + 1
  while (index < text.length) {
    const char = text[index]
    if (char === '\\') index += 2
    else if (char === quote) return index + 1
    else if (quote === '`' && char === '$' && text[index + 1] === '{')
      index = brace(text, index + 1) + 1
    else if (char === '\n' && quote !== '`') return index
    else index++
  }
  return text.length
}

/** Returns the offset of the brace that closes the one at `start`. */
function brace(code: string, start: number): number {
  let depth = 0
  for (let index = start; index < code.length; index++) {
    if (code[index] === '{') depth++
    if (code[index] === '}' && --depth === 0) return index
  }
  return code.length - 1
}

/** Returns the first argument of a call whose parenthesis opened just before `start`. */
function first(code: string, start: number): string {
  let depth = 1
  for (let index = start; index < code.length; index++) {
    if ('([{'.includes(code[index]!)) depth++
    if (')]}'.includes(code[index]!)) depth--
    if (depth === 0 || (depth === 1 && code[index] === ','))
      return code.slice(start, index)
  }
  return code.slice(start)
}

/** Joins consecutive own-line comments into one comment, which reads as one sentence or more. */
function merge(
  comments: { line: number; text: string }[],
): { line: number; text: string }[] {
  const merged: { line: number; text: string }[] = []
  for (const comment of comments) {
    const last = merged.at(-1)
    if (
      last &&
      last.line + last.text.split('\n').length === comment.line &&
      comment.text !== ''
    )
      last.text += `\n${comment.text}`
    else merged.push({ ...comment })
  }
  return merged.map((comment) => ({
    line: comment.line,
    text: comment.text.replaceAll('\n', ' ').trim(),
  }))
}

/** Returns the line of an offset, from the line starts of each text, computed once. */
function lineAt(text: string, offset: number): number {
  let starts = lineStarts.get(text)
  if (!starts) {
    starts = [0]
    for (let index = 0; index < text.length; index++)
      if (text[index] === '\n') starts.push(index + 1)
    lineStarts.set(text, starts)
  }
  let low = 0
  let high = starts.length - 1
  while (low < high) {
    const middle = (low + high + 1) >> 1
    if (starts[middle]! <= offset) low = middle
    else high = middle - 1
  }
  return low + 1
}

/** Returns the tracked and untracked files that match `patterns` and that Git does not ignore. */
function list(patterns: readonly string[]): string[] {
  const result = Bun.spawnSync(
    [
      'git',
      'ls-files',
      '--cached',
      '--others',
      '--exclude-standard',
      '-z',
      '--',
      ...patterns.flatMap((pattern) => [pattern, `**/${pattern}`]),
    ],
    { stderr: 'ignore' },
  )
  const found = result.success
    ? result.stdout.toString().split('\0')
    : patterns.flatMap((pattern) => [
        ...new Bun.Glob(`**/${pattern}`).scanSync(),
      ])
  return [...new Set(found)]
    .filter((file) => file && fs.existsSync(file))
    .sort()
}

/**
 * Returns the command-line file of an installed package, such as `node_modules/oxlint/bin/oxlint`,
 * or `undefined`. It looks in `node_modules` beside the script and then in each directory above
 * it, so the pack's own dependencies come before the project's. It reads the package, not
 * `node_modules/.bin`, where another tool, such as Vite Plus, may install a wrapper under the same
 * name.
 */
function bin(name: string): string | undefined {
  const directory = (() => {
    for (let at = import.meta.dir; ; at = path.dirname(at)) {
      const candidate = path.join(at, 'node_modules', name)
      if (fs.existsSync(path.join(candidate, 'package.json'))) return candidate
      if (path.dirname(at) === at) return undefined
    }
  })()
  if (!directory) return undefined
  const manifest = path.join(directory, 'package.json')

  const field = (
    JSON.parse(fs.readFileSync(manifest, 'utf8')) as {
      bin?: string | Record<string, string>
    }
  ).bin
  const file = typeof field === 'string' ? field : field?.[name]
  return file ? path.join(directory, file) : undefined
}

/**
 * Writes a check for one item: it passes without problems, and fails with the first as its note.
 */
function check(options: check.Options): void {
  const { item, metric, problems } = options
  const more = problems.length > 1 ? ` (and ${problems.length - 1} more)` : ''
  lines.push(
    problems.length === 0
      ? `${metric} ${quote(item)} pass`
      : `${metric} ${quote(item)} fail${note(`${problems[0]}${more}`)}`,
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
    item: string
    metric: string
    problems: readonly string[]
  }
}

declare namespace closing {
  /** The offset of the opening quote, and the quote character. */
  type Options = {
    quote: string
    start: number
  }
}
