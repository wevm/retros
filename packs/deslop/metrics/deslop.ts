/**
 * Measures the structure of source files with the four lints of deslop-please
 * (https://github.com/deterministic-machines/deslop-please): deep modules and narrow interfaces.
 * deslop-please reads Rust through rust-analyzer. The TypeScript checker resolves module, field,
 * and function references. A scanner reads the other source declarations and imports. A module
 * is a file, a Go package, or a C source with its header. Tests are not read. Each module is an
 * item, and a failing check carries the first finding.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'
import * as Imports from './Imports.js'
import * as Rust from './Rust.js'
import * as TypeScript from './TypeScript.js'

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
const roots = new Set(['lib.rs', 'main.rs', 'mod.rs'])

const candidates = list(
  Object.keys(languages).map((extension) => `*${extension}`),
).filter(
  (file) => !ignored.some((pattern) => pattern.test(file)) && !tests.test(file),
)
const rust = candidates.some((file) => file.endsWith('.rs'))
  ? Rust.workspace()
  : undefined
const files = candidates.filter(
  (file) =>
    !rust ||
    !file.endsWith('.rs') ||
    (!rust.diagnostics && rust.files.has(file)),
)
const known = new Set(files)
const lineStarts = new Map<string, number[]>()
const sources = new Map(files.map((file) => [file, parse(file)]))
const modules = [...new Set(files.map(moduleOf))]
const findings = new Map<string, Map<string, string[]>>(
  lints.map((lint) => [lint, new Map()]),
)
const skipped = new Map<string, Map<string, string>>(
  lints.map((lint) => [lint, new Map()]),
)

if (candidates.length === 0) {
  fs.appendFileSync(
    process.env.UPHILL_METRICS!,
    `${lints.map((lint) => `${lint} skip note "no source files"`).join('\n')}\n`,
  )
  process.exit(0)
}

const ts = TypeScript.analyze(
  files.filter((file) => /\.(?:m?ts|tsx)$/.test(file)),
)
if (ts.incomplete)
  for (const file of ts.edges.keys())
    for (const lint of lints) skipped.get(lint)!.set(file, ts.incomplete)
const imports = await Imports.analyze(files)
const graph = uses()
const extents = types()
const tangles = circles(typeGraph())

moduleCycles()
tangled()
fieldLeaks()
shallowModules()
for (const file of candidates.filter((file) => !known.has(file))) {
  modules.push(file)
  if (!rust!.files.has(file))
    for (const lint of lints) skipped.get(lint)!.set(file, rust!.reason)
}
if (rust?.diagnostics) nativeFindings(rust.diagnostics)

const lines = lints.flatMap((lint) =>
  modules.map((module) => {
    const backend =
      module.endsWith('.rs') || module === 'Cargo.toml'
        ? (rust?.label ?? 'Rust source scanner')
        : ts.edges.has(module)
          ? 'TypeScript source program'
          : 'source scanner'
    const problems = findings.get(lint)!.get(module) ?? []
    const first = problems[0]
    const prefix = `${lint} ${quote(module)}`
    const reason = skipped.get(lint)!.get(module)
    if (!first)
      return reason
        ? `${prefix} skip note ${quote(`${backend}: ${reason}`)}`
        : `${prefix} pass note ${quote(backend)}`
    const more =
      problems.length > heldInMind
        ? ` (and ${problems.length - heldInMind} more)`
        : ''
    return `${prefix} fail note ${quote(`${backend}: ${problems.slice(0, heldInMind).join(' | ')}${more}`)}`
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

/** Returns the directory a Rust module encloses, as `a.rs` encloses `a/b.rs`. */
function scopeOf(module: string): string {
  if (roots.has(path.basename(module))) return path.dirname(module)
  return module.replace(/\.\w+$/, '')
}

/**
 * Returns whether one module encloses the other. A module and the modules inside it form no
 * circle, as deslop-please counts circles only among siblings.
 */
function related(a: string, b: string): boolean {
  if (!a.endsWith('.rs') || !b.endsWith('.rs')) return false
  const encloses = (outer: string, inner: string) =>
    outer !== inner &&
    (scopeOf(outer) === '.' || `${inner}/`.startsWith(`${scopeOf(outer)}/`))
  return encloses(a, b) || encloses(b, a)
}

/** Returns, by module, the other modules its code imports or names. */
function uses(): Map<string, Set<string>> {
  const edges = new Map(modules.map((module) => [module, new Set<string>()]))
  const goModules = list(['go.mod'])
    .map((file) => ({
      dir: path.dirname(file),
      name:
        /^\s*module\s+"?([^"\s]+)/m.exec(fs.readFileSync(file, 'utf8'))?.[1] ??
        '',
    }))
    .sort((a, b) => b.name.length - a.name.length)
  const unresolved = new Map<Language, string>()

  for (const file of files) {
    const source = sources.get(file)!
    const dir = path.dirname(file)
    const from = moduleOf(file)
    const targets = [...(ts.edges.get(file) ?? [])]
    const unknown = (description: string) =>
      unresolved.set(source.language, `${file}: ${description}`)

    const incomplete = imports.incomplete.get(file)
    if (incomplete) unknown(incomplete)
    if (incomplete === 'syntax error')
      for (const lint of lints)
        skipped.get(lint)!.set(from, `${file}: ${incomplete}`)
    if (source.extension === '.rs') {
      if (/\bmod\s+\w+\s*\{/.test(source.code))
        skipped
          .get('module-cycle')!
          .set(file, 'inline Rust modules require deslop-please')
      if (/\b(?:crate|self|super)\s*::\s*\{|\binclude!\s*\(/.test(source.code))
        unknown('grouped root paths and include! require deslop-please')
      for (const match of source.code.matchAll(
        /\b(crate|super|self)((?:\s*::\s*\w+)+)/g,
      )) {
        const target = resolveRust(
          file,
          match[1]!,
          match[2]!.replaceAll(/\s/g, '').slice(2),
        )
        if (target.length === 0) unknown(`unresolved Rust path ${match[0]}`)
        targets.push(...target)
      }
    }
    for (const specifier of imports.edges.get(file) ?? []) {
      if (source.extension === '.go') {
        const owner = goModules.find(
          (module) =>
            module.name &&
            (specifier === module.name ||
              specifier.startsWith(`${module.name}/`)),
        )
        if (!owner) {
          if (goModules.length === 0) unknown('no go.mod for import resolution')
          continue
        }
        const to = `${path.join(owner.dir, specifier.slice(owner.name.length))}/`
        if (!edges.has(to)) unknown(`unresolved package ${specifier}`)
        else if (from !== to) edges.get(from)!.add(to)
      } else
        targets.push(
          source.extension === '.swift' ? specifier : path.join(dir, specifier),
        )
    }
    for (const target of targets) {
      const to = moduleOf(target)
      if (!known.has(target)) unknown(`unresolved module ${target}`)
      else if (to !== from) edges.get(from)!.add(to)
    }
  }

  for (const [file, source] of sources) {
    const reason = unresolved.get(source.language)
    if (reason)
      for (const lint of ['module-cycle', 'shallow-module'])
        skipped.get(lint)!.set(moduleOf(file), reason)
  }
  return edges
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
    if (declaring.size !== 1) {
      found.delete(name)
      for (const module of declaring)
        for (const lint of ['field-leak', 'tangle'])
          skipped.get(lint)!.set(module, `ambiguous type name ${name}`)
    }
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
  for (const [name, list] of extents) {
    const declaration = list.find((extent) => extent.declaration)
    if (!declaration) continue
    const fields = fieldNames(declaration)
    const owner = moduleOf(declaration.file)
    const language = sources.get(declaration.file)!.language
    const tangle = tangles.find((members) => members.includes(name)) ?? [name]
    const inside = tangle.flatMap((member) => extents.get(member) ?? [])
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
    if (!works || fields.length < 2) continue

    const leaks = new Map<string, string[]>()
    for (const file of files) {
      const source = sources.get(file)!
      if (source.language !== language) continue
      const { code } = source
      for (const field of fields) {
        const pattern = new RegExp(
          `\\.\\s*${field}\\b(?=\\s*(?:[-+*/%|&^]?=(?!=)|\\+\\+|--|\\[|\\.\\w+\\s*\\())|&mut\\s+[\\w.]+\\.${field}\\b`,
          'g',
        )
        for (const match of code.matchAll(pattern)) {
          const insider = inside.some(
            (extent) =>
              extent.file === file &&
              match.index >= extent.start &&
              match.index < extent.end,
          )
          if (insider) continue
          const at = match.index + match[0].length - field.length
          const place = `${file}:${lineAt(code, at)}`
          const receiver = ts.edges.has(file)
            ? ts.properties.get(file)?.get(at)
            : receiverType(source, at)
          if (receiver === undefined)
            skipped
              .get('field-leak')!
              .set(owner, `unresolved receiver of ${field} at ${place}`)
          else if (
            receiver ===
            (ts.edges.has(file) ? `${declaration.file}:${name}` : name)
          )
            leaks.set(field, [...(leaks.get(field) ?? []), place])
        }
      }
    }
    if (leaks.size === 0 || leaks.size === fields.length) continue

    for (const [field, places] of leaks)
      find(
        'field-leak',
        owner,
        `\`${name}.${field}\` in ${owner} accessed at ${listed(places)}`,
      )
  }
}

/** Resolves simple annotated receivers. A spelling without a binding is not type evidence. */
function receiverType(
  source: ReturnType<typeof parse>,
  at: number,
): string | undefined {
  const prefix = source.code.slice(0, at)
  const receiver = /(?<![\w.])\b(\w+)\s*\.\s*$/.exec(prefix)?.[1]
  if (!receiver) return undefined
  const start = source.functions.findLast((fn) => fn.offset < at)?.offset ?? 0
  const scope = prefix.slice(start)
  const pattern =
    source.extension === '.go'
      ? `\\b${receiver}\\s+\\*?(?:\\w+\\.)?(\\w+)`
      : `\\b${receiver}\\s*:\\s*(?:[&*]\\s*)?(?:'\\w+\\s+)?(?:mut\\s+|const\\s+|inout\\s+)?(?:\\w+(?:::|\\.))*(\\w+)`
  const bindings = [...scope.matchAll(new RegExp(pattern, 'g'))]
  if (
    bindings.length !== 1 ||
    new RegExp(`\\b(?:let|var|const)\\s+(?:mut\\s+)?${receiver}\\s*=`).test(
      scope,
    )
  )
    return undefined
  return bindings[0]![1]
}

/** Returns the field names that a type's declaration lists at the top level of its body. */
function fieldNames(extent: Extent): string[] {
  if (ts.edges.has(extent.file))
    return [...(ts.fieldsOf.get(`${extent.file}:${extent.name}`) ?? [])]
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
  for (const module of modules) {
    const names = defined.get(module)!
    if (names.size <= heldInMind) continue
    const callers = [...users.get(module)!]
    const code = files
      .filter((file) => callers.includes(moduleOf(file)))
      .map((file) => sources.get(file)!.code)
      .join('\n')
    const shown = ts.edges.has(module)
      ? [...ts.shown.get(module)!].filter((name) => names.has(name))
      : [...names].filter(
          (name) =>
            callers.some((caller) => !defined.get(caller)?.has(name)) &&
            new RegExp(`(?<![\\w])${name}\\s*\\(`).test(code),
        )
    const hidden = names.size - shown.length
    if (ts.edges.has(module) && ts.unknownCall)
      skipped.get('shallow-module')!.set(module, ts.unknownCall)
    if (shown.length > heldInMind && hidden <= shown.length)
      find(
        'shallow-module',
        module,
        `shows ${shown.length} of its ${names.size} functions: ${listed(shown)}`,
      )
  }
}

/** Attributes native circles to their reported files and leaks to the type, not the first access. */
function nativeFindings(output: string): void {
  const measured = modules.filter((file) => rust!.files.has(file))

  for (const block of output.split(/(?=^error\[)/m)) {
    const header = /^error\[([\w-]+)\]: (.+)/.exec(block)
    const lint = lints.find((lint) => lint === header?.[1])
    if (!lint) continue
    const sites = [
      ...block.matchAll(/^  (?:-->|= note:) (.+?\.rs):(\d+):\d+/gm),
    ]
    const notes = [...block.matchAll(/^  = note: (.+)/gm)].map(
      (note) => note[1],
    )
    const text = `${header![2]}; ${notes.join('; ') || sites[0]?.[0].trim() || 'no source location'}`
    const files = new Set(sites.map((site) => site[1]!))
    if (lint === 'field-leak') {
      files.clear()
      const type = /^    ([\w:]+)\s*$/m.exec(block)?.[1]
      const declarations = type && rust!.owners.get(type)
      if (declarations && declarations.size === 1)
        for (const owner of declarations) files.add(owner)
    }
    if (
      files.size === 0 ||
      /\band \d+ more (?:tie|place|module)|; also at /m.test(block)
    )
      for (const module of measured)
        skipped
          .get(lint)!
          .set(module, 'native diagnostic has incomplete file attribution')

    if (files.size === 0) files.add('Cargo.toml')
    for (const file of files) {
      if (!modules.includes(file)) {
        modules.push(file)
        for (const other of lints)
          skipped.get(other)!.set(file, 'diagnostic location only')
      }
      find(lint, file, text)
    }
  }
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
      if (char === '`' && extension === '.go') {
        const stop = text.indexOf('`', index + 1)
        return stop === -1 ? text.length : stop + 1
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
  return { code: blanked, extension, functions, language }
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
