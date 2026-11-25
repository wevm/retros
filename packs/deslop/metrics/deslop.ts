/**
 * Measures the structure of source files with the four lints of deslop-please
 * (https://github.com/deterministic-machines/deslop-please): deep modules and narrow interfaces.
 * deslop-please reads Rust through rust-analyzer, so it measures Rust when it is installed. No tool
 * reads the other languages, so a small scanner removes comments and strings, matches braces, and
 * reads imports, types, fields, and functions in Rust, Zig, Go, Swift, C, and TypeScript. A module
 * is a file, a Go package, or a C source with its header. Tests are not read. Each module is an
 * item, and a failing check carries the first finding.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

/** The lints, in the order deslop-please lists them. Each lint is a metric. */
const lints = [
  'module-cycle',
  'tangle',
  'field-leak',
  'shallow-module',
] as const

/**
 * What a reader holds in mind at once, as deslop-please counts it: seven (Miller, 1956). A tangle
 * of more members fails in one module, and a module that shows more functions may be shallow.
 */
const heldInMind = 7

/** File extensions the scanner reads, with the syntax of each language. */
const languages: Record<string, Language> = {
  '.c': {
    functions: /^[\w*\s]+?\b(\w+)\s*\([^;{}]*\)\s*\{/gm,
    types: /\b(?:struct|union|enum)\s+(\w+)\s*\{/g,
  },
  '.go': {
    functions: /\bfunc\s+(?:\([^)]*\)\s*)?(\w+)\s*[[(]/g,
    types: /\btype\s+(\w+)\s+(?:struct|interface)\s*\{/g,
  },
  '.rs': {
    functions: /\bfn\s+(\w+)/g,
    types: /\b(?:struct|enum|trait|union)\s+(\w+)/g,
  },
  '.swift': {
    functions: /\bfunc\s+(\w+)/g,
    types: /\b(?:class|struct|enum|protocol|actor|extension)\s+(\w+)/g,
  },
  '.ts': {
    functions:
      /\bfunction\s*\*?\s*(\w+)|\b(?:const|let)\s+(\w+)\s*=\s*(?:async\s*)?(?:\([^()]*\)|\w+)\s*(?::[^=]+)?=>|^\s*(?:(?:public|private|protected|static|async|override|readonly)\s+)*(?!(?:if|for|while|switch|catch|with|return|function)\b)(\w+)\s*\([^()]*\)\s*(?::\s*[^{;]+)?\{/gm,
    types: /\b(?:class|interface|enum)\s+(\w+)|\btype\s+(\w+)\s*=\s*\{/g,
  },
  '.zig': {
    functions: /\bfn\s+(\w+)\s*\(/g,
    types:
      /\bconst\s+(\w+)\s*=\s*(?:extern\s+|packed\s+)?(?:struct|union|enum)\b/g,
  },
}
languages['.h'] = languages['.c']!
languages['.mts'] = languages['.ts']!
languages['.tsx'] = languages['.ts']!

/** Paths that are not maintained source: generated code, installed packages, and vendored packs. */
const ignored = [
  /(^|\/)(\.agents\/packs|dist|build|node_modules|target|vendor|zig-cache|\.zig-cache|zig-out)\//,
  /\.d\.ts$/,
]
const tests =
  /(^|\/)(tests?|__tests__|benches)\/|[._](test|spec)\.\w+$|_test\.go$/

/** Words that open a block like a function header does, but are not functions. */
const keywords = new Set([
  'catch',
  'constructor',
  'else',
  'for',
  'if',
  'return',
  'sizeof',
  'switch',
  'while',
])

/** File names whose module encloses every module in their directory. */
const roots = new Set([
  'index.ts',
  'index.tsx',
  'lib.rs',
  'main.rs',
  'main.zig',
  'mod.rs',
  'root.zig',
])

const files = list(
  Object.keys(languages).map((extension) => `*${extension}`),
).filter(
  (file) => !ignored.some((pattern) => pattern.test(file)) && !tests.test(file),
)
const known = new Set(files)
const lineStarts = new Map<string, number[]>()
const sources = new Map(files.map((file) => [file, parse(file)]))
const modules = [...new Set(files.map(moduleOf))]
const findings = new Map<string, Map<string, string[]>>(
  lints.map((lint) => [lint, new Map()]),
)

if (files.length === 0) {
  fs.appendFileSync(
    process.env.UPHILL_METRICS!,
    `${lints.map((lint) => `${lint} skip note "no source files"`).join('\n')}\n`,
  )
  process.exit(0)
}

const graph = uses()
const extents = types()
const tangles = circles(typeGraph())

moduleCycles()
tangled()
fieldLeaks()
shallowModules()
const rust = deslopPlease()

const lines = lints.flatMap((lint) =>
  modules.map((module) => {
    const problems =
      rust?.get(lint)?.get(module) ??
      (rust && module.endsWith('.rs')
        ? []
        : (findings.get(lint)!.get(module) ?? []))
    const first = problems[0]
    if (!first) return `${lint} ${quote(module)} pass`
    const more = problems.length > 1 ? ` (and ${problems.length - 1} more)` : ''
    return `${lint} ${quote(module)} fail note ${quote(`${first}${more}`)}`
  }),
)
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Records a finding of `lint` for `module`. */
function find(
  lint: (typeof lints)[number],
  module: string,
  text: string,
): void {
  const found = findings.get(lint)!
  found.set(module, [...(found.get(module) ?? []), text])
}

/** Returns the module of a file: the file, its Go package, or its C source and header together. */
function moduleOf(file: string): string {
  if (file.endsWith('.go')) return `${path.dirname(file)}/`
  if (/\.[ch]$/.test(file)) {
    const source = file.replace(/\.h$/, '.c')
    return known.has(source) ? source : file
  }
  return file
}

/**
 * Returns the directory or path prefix that a module encloses: a root file's directory, a Go
 * package, or the path without its extension, as `a.rs` encloses `a/b.rs`.
 */
function scopeOf(module: string): string {
  if (module.endsWith('/')) return module.slice(0, -1)
  if (roots.has(path.basename(module))) return path.dirname(module)
  return module.replace(/\.\w+$/, '')
}

/**
 * Returns whether one module encloses the other. A module and the modules inside it form no
 * circle, as deslop-please counts circles only among siblings.
 */
function related(a: string, b: string): boolean {
  const encloses = (outer: string, inner: string) =>
    outer !== inner &&
    (scopeOf(outer) === '.' || `${inner}/`.startsWith(`${scopeOf(outer)}/`))
  return encloses(a, b) || encloses(b, a)
}

/** Returns, by module, the other modules its code imports or names. */
function uses(): Map<string, Set<string>> {
  const edges = new Map(modules.map((module) => [module, new Set<string>()]))
  const goModules = list(['go.mod']).map((file) => ({
    dir: path.dirname(file),
    name: /^module\s+(\S+)/m.exec(fs.readFileSync(file, 'utf8'))?.[1] ?? '',
  }))

  for (const file of files) {
    const source = sources.get(file)!
    const dir = path.dirname(file)
    const targets: string[] = []
    const live = (index: number) => source.code[index] === source.text[index]

    if (source.extension.endsWith('ts') || source.extension === '.tsx') {
      const pattern =
        /\b(?:import|export)\s+(?:type\s+)?[\w\s{},*$]*?\bfrom\s*(['"])(\.{1,2}\/[^'"]+)\1|\bimport\s*\(\s*(['"])(\.{1,2}\/[^'"]+)\3\s*\)/g
      for (const match of source.text.matchAll(pattern))
        if (live(match.index))
          targets.push(...resolveScript(path.join(dir, match[2] ?? match[4]!)))
    }
    if (source.extension === '.rs')
      for (const match of source.code.matchAll(
        /\b(crate|super|self)((?:::\w+)+)/g,
      ))
        targets.push(...resolveRust(file, match[1]!, match[2]!.slice(2)))
    if (source.extension === '.zig')
      for (const match of source.text.matchAll(
        /@import\(\s*"([^"]+\.zig)"\s*\)/g,
      ))
        if (live(match.index)) targets.push(path.join(dir, match[1]!))
    if (source.extension === '.c' || source.extension === '.h')
      for (const match of source.text.matchAll(/^\s*#\s*include\s+"([^"]+)"/gm))
        targets.push(path.join(dir, match[1]!))
    if (source.extension === '.go')
      for (const match of source.text.matchAll(/"([\w.\-/]+)"/g)) {
        const owner = goModules.find(
          (module) => module.name && match[1]!.startsWith(`${module.name}/`),
        )
        if (owner)
          targets.push(
            `${path.join(owner.dir, match[1]!.slice(owner.name.length + 1))}/x.go`,
          )
      }

    const from = moduleOf(file)
    for (const target of targets) {
      const to = target.endsWith('/x.go')
        ? `${path.dirname(target)}/`
        : known.has(target)
          ? moduleOf(target)
          : undefined
      if (to && to !== from && edges.has(to)) edges.get(from)!.add(to)
    }
  }
  return edges
}

/** Returns the files that a relative TypeScript import names, trying each way it resolves. */
function resolveScript(target: string): string[] {
  const stem = target.replace(/\.[mc]?[jt]sx?$/, '')
  return [
    target,
    `${stem}.ts`,
    `${stem}.tsx`,
    `${stem}.mts`,
    `${target}/index.ts`,
    `${target}/index.tsx`,
  ]
    .filter((candidate) => known.has(candidate))
    .slice(0, 1)
}

/** Returns the file of the longest module path that a Rust path names, such as `crate::a::b`. */
function resolveRust(file: string, start: string, rest: string): string[] {
  const own = roots.has(path.basename(file))
    ? path.dirname(file)
    : file.replace(/\.rs$/, '')
  const base = (() => {
    if (start === 'self') return own
    if (start === 'super') return path.dirname(own)
    for (let dir = path.dirname(file); ; dir = path.dirname(dir)) {
      if (
        known.has(path.join(dir, 'lib.rs')) ||
        known.has(path.join(dir, 'main.rs'))
      )
        return dir
      if (dir === '.' || dir === path.dirname(dir)) return path.dirname(file)
    }
  })()
  const segments = rest.split('::')
  for (let count = segments.length; count > 0; count--) {
    const at = path.join(base, ...segments.slice(0, count))
    for (const candidate of [`${at}.rs`, `${at}/mod.rs`])
      if (known.has(candidate)) return [candidate]
  }
  return []
}

/** Writes `module-cycle`: modules that use each other in a circle, apart from enclosing ones. */
function moduleCycles(): void {
  const unrelated = new Map(
    [...graph].map(([from, to]) => [
      from,
      new Set([...to].filter((other) => !related(from, other))),
    ]),
  )
  for (const circle of circles(unrelated)) {
    const names = listed(circle)
    for (const module of circle)
      find('module-cycle', module, `${names} use each other in a circle`)
  }
}

/**
 * Returns each type's extents: its declaration, and the `impl` blocks, receivers, and extensions
 * that add methods to it. A type name that two modules declare is left out, since a reference
 * cannot say which it names.
 */
function types(): Map<string, Extent[]> {
  const found = new Map<string, Extent[]>()
  const add = (extent: Extent) =>
    found.set(extent.name, [...(found.get(extent.name) ?? []), extent])

  for (const file of files) {
    const source = sources.get(file)!
    for (const match of source.code.matchAll(source.language.types)) {
      const name = match.slice(1).find(Boolean)!
      const open = source.code.indexOf('{', match.index)
      const stop = source.code.slice(match.index, open).indexOf(';')
      const declared =
        open !== -1 && (stop === -1 || /\btype\b|\bconst\b/.test(match[0]))
      add({
        declaration: !match[0].startsWith('extension'),
        end: declared
          ? brace(source.code, open)
          : match.index + match[0].length,
        file,
        name,
        receiver: undefined,
        start: declared ? open + 1 : match.index,
        traitImpl: false,
      })
    }
    if (source.extension === '.rs')
      for (const match of source.code.matchAll(
        /\bimpl\b(?:\s*<[^{]*?>)?\s+(?:([\w:<>,\s&'+]+?)\s+for\s+)?(?:[\w:]+::)?(\w+)[^{;]*\{/g,
      )) {
        const open = match.index + match[0].length - 1
        add({
          declaration: false,
          end: brace(source.code, open),
          file,
          name: match[2]!,
          receiver: 'self',
          start: open + 1,
          traitImpl: Boolean(match[1]),
        })
      }
    if (source.extension === '.go')
      for (const match of source.code.matchAll(
        /\bfunc\s*\(\s*(\w+)\s+\*?(\w+)[^)]*\)\s*\w+[^{]*\{/g,
      )) {
        const open = match.index + match[0].length - 1
        add({
          declaration: false,
          end: brace(source.code, open),
          file,
          name: match[2]!,
          receiver: match[1]!,
          start: open + 1,
          traitImpl: false,
        })
      }
  }

  for (const [name, list] of found) {
    const declaring = new Set(
      list
        .filter((extent) => extent.declaration)
        .map((extent) => moduleOf(extent.file)),
    )
    if (declaring.size !== 1) found.delete(name)
  }
  return found
}

/** Returns, by type, the other project types that its extents name. */
function typeGraph(): Map<string, Set<string>> {
  const edges = new Map<string, Set<string>>()
  for (const [name, list] of extents) {
    const named = new Set<string>()
    for (const extent of list) {
      const code = sources
        .get(extent.file)!
        .code.slice(extent.start, extent.end)
      for (const match of code.matchAll(/\b[A-Za-z_]\w*\b/g))
        if (match[0] !== name && extents.has(match[0])) named.add(match[0])
    }
    edges.set(name, named)
  }
  return edges
}

/**
 * Writes `tangle`: types that know each other in a circle, written in more than one module or
 * more of them than a reader holds in mind.
 */
function tangled(): void {
  for (const members of tangles) {
    const where = new Set(
      members.flatMap((name) =>
        extents.get(name)!.map((extent) => moduleOf(extent.file)),
      ),
    )
    if (where.size < 2 && members.length <= heldInMind) continue
    const text = `${listed(members)} know each other in a circle across ${listed([...where])}`
    for (const module of where) find('tangle', module, text)
  }
}

/**
 * Writes `field-leak`: a field that code outside its type writes, indexes, or calls a method on,
 * when the type keeps another field to itself and a method of its own uses its fields. Reading a
 * value is no leak. The type's methods and the types in its tangle are inside it.
 */
function fieldLeaks(): void {
  const declared = new Map<string, string[]>()
  const fieldsOf = new Map<string, string[]>()
  for (const [name, list] of extents) {
    const declaration = list.find((extent) => extent.declaration)
    if (!declaration) continue
    const fields = fieldNames(declaration)
    fieldsOf.set(name, fields)
    for (const field of fields)
      declared.set(field, [...(declared.get(field) ?? []), name])
  }

  for (const [name, fields] of fieldsOf) {
    const list = extents.get(name)!
    const tangle = tangles.find((members) => members.includes(name)) ?? [name]
    const inside = tangle.flatMap((member) => extents.get(member) ?? [])
    const unique = fields.filter((field) => declared.get(field)?.length === 1)
    const works = list.some((extent) => {
      const code = sources
        .get(extent.file)!
        .code.slice(extent.start, extent.end)
      const receiver =
        extent.receiver ?? (extent.file.endsWith('.rs') ? 'self' : 'this|self')
      return fields.some((field) =>
        new RegExp(`\\b(?:${receiver})\\.${field}\\b`).test(code),
      )
    })
    if (!works || unique.length === 0) continue

    const leaks = new Map<string, string[]>()
    for (const file of files) {
      const code = sources.get(file)!.code
      for (const field of unique) {
        const pattern = new RegExp(
          `\\.${field}\\b(?=\\s*(?:[-+*/%|&^]?=(?!=)|\\+\\+|--|\\[|\\.\\w+\\s*\\())|&mut\\s+[\\w.]+\\.${field}\\b`,
          'g',
        )
        for (const match of code.matchAll(pattern)) {
          const insider = inside.some(
            (extent) =>
              extent.file === file &&
              match.index >= extent.start &&
              match.index < extent.end,
          )
          if (!insider)
            leaks.set(field, [
              ...(leaks.get(field) ?? []),
              `${file}:${lineAt(code, match.index)}`,
            ])
        }
      }
    }
    if (leaks.size === 0 || leaks.size === fields.length) continue

    const module = moduleOf(list.find((extent) => extent.declaration)!.file)
    for (const [field, places] of leaks)
      find(
        'field-leak',
        module,
        `\`${name}.${field}\` is known outside \`${name}\`: ${places.length} ${places.length === 1 ? 'place' : 'places'}, first ${places[0]}`,
      )
  }
}

/** Returns the field names that a type's declaration lists at the top level of its body. */
function fieldNames(extent: Extent): string[] {
  const source = sources.get(extent.file)!
  const body = source.code.slice(extent.start, extent.end)
  let depth = 0
  let top = ''
  for (const char of body) {
    if (char === '{' || char === '(') depth++
    if (char === '}' || char === ')') depth--
    if (depth === 0 && char !== '}' && char !== ')') top += char
  }
  const pattern = (() => {
    if (source.extension === '.go') return /^\s*(\w+)\s+[\w*[\]]/gm
    if (source.extension === '.swift') return /\b(?:var|let)\s+(\w+)/g
    if (source.extension === '.c' || source.extension === '.h')
      return /\b(\w+)\s*(?:\[[^\]]*\])?\s*;/g
    if (source.extension === '.rs' || source.extension === '.zig')
      return /^\s*(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:/gm
    return /^\s*(?:(?:public|private|protected|readonly|static|declare)\s+)*#?(\w+)\s*[?!]?\s*[:=](?!=)/gm
  })()
  return [
    ...new Set([...top.matchAll(pattern)].map((match) => match[1]!)),
  ].filter((field) => !keywords.has(field) && field !== 'fn')
}

/**
 * Writes `shallow-module`: a module that shows more functions than a reader holds in mind, and
 * hides no more than it shows. It shows a function when a module that uses it calls the function,
 * apart from methods of a trait's `impl` block.
 */
function shallowModules(): void {
  const defined = new Map<string, Set<string>>()
  for (const file of files) {
    const source = sources.get(file)!
    const traitImpls = [...extents.values()]
      .flat()
      .filter((extent) => extent.file === file && extent.traitImpl)
    const names = defined.get(moduleOf(file)) ?? new Set<string>()
    for (const fn of source.functions)
      if (
        !traitImpls.some(
          (extent) => fn.offset >= extent.start && fn.offset < extent.end,
        )
      )
        names.add(fn.name)
    defined.set(moduleOf(file), names)
  }

  const users = new Map(modules.map((module) => [module, new Set<string>()]))
  for (const [from, to] of graph)
    for (const module of to) users.get(module)!.add(from)
  for (const module of modules)
    if (module.endsWith('.swift'))
      for (const other of modules)
        if (other !== module && other.endsWith('.swift'))
          users.get(module)!.add(other)

  for (const module of modules) {
    const names = defined.get(module)!
    const callers = [...users.get(module)!]
    const code = files
      .filter((file) => callers.includes(moduleOf(file)))
      .map((file) => sources.get(file)!.code)
      .join('\n')
    const shown = [...names].filter(
      (name) =>
        callers.some((caller) => !defined.get(caller)?.has(name)) &&
        new RegExp(`(?<![\\w])${name}\\s*\\(`).test(code),
    )
    const hidden = names.size - shown.length
    if (shown.length > heldInMind && hidden <= shown.length)
      find(
        'shallow-module',
        module,
        `shows ${shown.length} of its ${names.size} functions: ${listed(shown)}`,
      )
  }
}

/**
 * Runs deslop-please on a Cargo workspace when it is installed, and returns its findings by lint
 * and file. A diagnostic without a location belongs to the `workspace` item. If
 * `.deslop-please.allow` exists, the tool reads it, so listed debt passes and new errors still
 * fail.
 */
function deslopPlease(): Map<string, Map<string, string[]>> | undefined {
  if (
    !fs.existsSync('Cargo.toml') ||
    !files.some((file) => file.endsWith('.rs'))
  )
    return undefined
  const allow = '.deslop-please.allow'
  const result = (() => {
    try {
      return Bun.spawnSync(
        ['deslop-please', ...(fs.existsSync(allow) ? ['--allow', allow] : [])],
        {
          env: { ...process.env, NO_COLOR: '1' },
          stderr: 'pipe',
          stdout: 'pipe',
        },
      )
    } catch {
      return undefined
    }
  })()
  if (!result) return undefined
  const output = `${result.stdout.toString()}${result.stderr.toString()}`

  // A completed run ends with `deslop-please: ok` or `deslop-please: N errors`. Any other end means
  // it could not index the workspace, so the run fails instead of passing every file.
  if (!/^deslop-please: (ok|\d+ errors?\b)/m.test(output)) {
    process.stderr.write(output)
    process.exit(1)
  }

  const found = new Map<string, Map<string, string[]>>(
    lints.map((lint) => [lint, new Map()]),
  )
  for (const match of output.matchAll(
    /^error\[([\w-]+)\]: (.*)\n(?:\s+--> (.+?):(\d+):\d+)?/gm,
  )) {
    const byFile = found.get(match[1]!)
    if (!byFile) continue
    const file = match[3] ?? 'workspace'
    if (!modules.includes(file)) modules.push(file)
    const text = match[4] ? `line ${match[4]}: ${match[2]}` : match[2]!
    byFile.set(file, [...(byFile.get(file) ?? []), text])
  }
  return found
}

/** Returns the strongly connected groups of more than one node, in first-seen order (Tarjan). */
function circles(edges: Map<string, Set<string>>): string[][] {
  const index = new Map<string, number>()
  const low = new Map<string, number>()
  const stack: string[] = []
  const found: string[][] = []

  for (const root of edges.keys()) {
    if (index.has(root)) continue
    // An explicit stack, so a long chain of modules cannot overflow the call stack.
    const work: { node: string; next: string[] }[] = []
    const enter = (node: string) => {
      index.set(node, index.size)
      low.set(node, index.get(node)!)
      stack.push(node)
      work.push({ next: [...(edges.get(node) ?? [])], node })
    }
    enter(root)
    while (work.length > 0) {
      const top = work.at(-1)!
      const next = top.next.shift()
      if (next !== undefined) {
        if (!index.has(next)) enter(next)
        else if (stack.includes(next))
          low.set(top.node, Math.min(low.get(top.node)!, index.get(next)!))
        continue
      }
      work.pop()
      const parent = work.at(-1)
      if (parent)
        low.set(
          parent.node,
          Math.min(low.get(parent.node)!, low.get(top.node)!),
        )
      if (low.get(top.node) === index.get(top.node)) {
        const group = stack.splice(stack.indexOf(top.node))
        if (group.length > 1) found.push(group)
      }
    }
  }
  return found
}

/** Names up to seven things in backticks, with a count of the rest. */
function listed(names: readonly string[]): string {
  const shown = names.slice(0, heldInMind).map((name) => `\`${name}\``)
  const rest = names.length - shown.length
  return rest > 0 ? `${shown.join(', ')} and ${rest} more` : shown.join(', ')
}

/** The syntax of one language: how it declares functions and types. */
type Language = { functions: RegExp; types: RegExp }

/**
 * One place that adds to a type: its declaration, or an `impl` block, receiver, or extension, with
 * the offsets of its body and the name its methods give the value.
 */
type Extent = {
  declaration: boolean
  end: number
  file: string
  name: string
  receiver: string | undefined
  start: number
  traitImpl: boolean
}

/**
 * Reads a file, and returns its text, its code with comments and strings blanked out, and its
 * functions. Blanking keeps every offset and newline, so lines match the file.
 */
function parse(file: string) {
  const text = fs.readFileSync(file, 'utf8')
  const extension = path.extname(file)
  const language = languages[extension]!
  const code = text.split('')
  const blank = (from: number, to: number) => {
    for (let index = from; index < to; index++)
      if (code[index] !== '\n') code[index] = ' '
  }

  let index = 0
  while (index < code.length) {
    const char = code[index]!
    const next = code[index + 1]
    const end = (() => {
      if (char === '/' && next === '/') {
        const stop = text.indexOf('\n', index)
        return stop === -1 ? text.length : stop
      }
      if (char === '/' && next === '*') {
        const stop = text.indexOf('*/', index + 2)
        return stop === -1 ? text.length : stop + 2
      }
      if (char === "'" && extension === '.rs') {
        // A char literal closes within a few characters, and a lifetime does not close at all.
        const literal =
          /^'(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'])'/u.exec(
            text.slice(index),
          )
        return literal ? index + literal[0].length : undefined
      }
      if (char === '"' || char === "'" || char === '`')
        return closing(text, { quote: char, start: index })
      return undefined
    })()
    if (end === undefined) {
      index++
      continue
    }
    blank(index, end)
    index = end
  }

  const blanked = code.join('')
  const functions: { name: string; offset: number }[] = []
  for (const match of blanked.matchAll(language.functions)) {
    const name = match.slice(1).find(Boolean)
    if (name && !keywords.has(name))
      functions.push({ name, offset: match.index })
  }
  return { code: blanked, extension, functions, language, text }
}

/**
 * Returns the offset after the quote that closes a string, skipping escapes and template
 * expressions.
 */
function closing(text: string, options: closing.Options): number {
  let index = options.start + 1
  while (index < text.length) {
    const char = text[index]
    if (char === '\\') index += 2
    else if (char === options.quote) return index + 1
    else if (options.quote === '`' && char === '$' && text[index + 1] === '{')
      index = brace(text, index + 1) + 1
    else if (char === '\n' && options.quote !== '`') return index
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

/** Quotes an item or note for a metric line, escaping `"` and `\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"').replaceAll('\n', ' ')}"`
}

declare namespace closing {
  /** The offset of the opening quote, and the quote character. */
  type Options = {
    quote: string
    start: number
  }
}
