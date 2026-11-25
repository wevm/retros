/** @module Reads file, package and Swift symbol references from source syntax trees. */
import * as fs from 'node:fs'
import * as path from 'node:path'
import { Language, type Node, Parser } from 'web-tree-sitter'

const grammars = {
  '.c': 'c',
  '.go': 'go',
  '.h': 'c',
  '.swift': 'swift',
  '.zig': 'zig',
} as const
type Grammar = (typeof grammars)[keyof typeof grammars]

/** Collects import targets, or a reason the graph of that language is incomplete. */
export async function analyze(files: readonly string[]) {
  const edges = new Map<string, Set<string>>()
  const incomplete = new Map<string, string>()
  const parsers = new Map<Grammar, Parser>()
  const swift = new Map<string, string[]>()
  const calls = new Map<string, Set<string>>()
  const types = new Map<string, Set<string>>()
  const shadows = new Map<string, Set<string>>()

  for (const file of files) {
    const ext = path.extname(file)
    if (!Object.hasOwn(grammars, ext)) continue
    const grammar = grammars[ext as keyof typeof grammars]
    let parser = parsers.get(grammar)
    if (!parser) {
      if (parsers.size === 0) await Parser.init()
      const url = import.meta.resolve(
        `tree-sitter-wasms/out/tree-sitter-${grammar}.wasm`,
      )
      parser = new Parser().setLanguage(
        await Language.load(Bun.fileURLToPath(url)),
      )
      parsers.set(grammar, parser)
    }
    const tree = parser.parse(fs.readFileSync(file, 'utf8') + '\n')
    if (!tree) throw Error(`could not parse ${file}`)
    const root = tree.rootNode
    try {
      const imports = new Set<string>()
      edges.set(file, imports)
      if (root.hasError) {
        incomplete.set(file, 'syntax error')
        continue
      }
      const literal = (node: Node | null | undefined) => {
        if (!node || !/^(?:"[^"\\]*"|`[^`]*`)$/.test(node.text)) {
          incomplete.set(
            file,
            `unresolved import${node ? ` at line ${node.startPosition.row + 1}` : ''}`,
          )
          return
        }
        const target = node.text.slice(1, -1)
        if (grammar === 'zig' && !target.endsWith('.zig')) {
          if (target !== 'std' && target !== 'builtin')
            incomplete.set(file, `build-defined Zig import ${target}`)
        } else imports.add(target)
      }
      if (grammar === 'go') {
        for (const spec of root.descendantsOfType('import_spec'))
          literal(spec?.childForFieldName('path'))
      } else if (grammar === 'c') {
        for (const directive of root.descendantsOfType('preproc_include')) {
          if (directive?.parent?.type !== 'translation_unit') {
            incomplete.set(
              file,
              'conditional include requires preprocessor configuration',
            )
            continue
          }
          literal(directive.childForFieldName('path'))
        }
      } else if (grammar === 'zig') {
        for (const builtin of root.descendantsOfType('builtin_function')) {
          if (builtin?.firstNamedChild?.text !== '@import') continue
          const args = builtin.lastNamedChild?.namedChildren.filter(
            (arg) => arg && arg.type !== 'comment',
          )
          literal(
            args?.length === 1 && args[0]?.type === 'string'
              ? args[0]
              : undefined,
          )
        }
      } else {
        const local = new Set<string>()
        shadows.set(file, local)
        for (const decl of root.descendantsOfType([
          'function_declaration',
          'class_declaration',
          'protocol_declaration',
          'parameter',
          'property_declaration',
        ])) {
          const name = decl?.childForFieldName('name')?.text
          if (!decl || !name) continue
          if (decl.parent?.type !== 'source_file') local.add(name)
          else swift.set(name, [...(swift.get(name) ?? []), file])
        }
        const names = new Set<string>()
        calls.set(file, names)
        for (const call of root.descendantsOfType('call_expression')) {
          const callee = call?.firstNamedChild
          if (callee?.type === 'simple_identifier') names.add(callee.text)
          else incomplete.set(file, 'qualified or indirect Swift call')
        }
        types.set(
          file,
          new Set(
            root
              .descendantsOfType('user_type')
              .flatMap((type) => (type ? [type.text] : [])),
          ),
        )
      }
    } finally {
      tree.delete()
    }
  }
  for (const parser of parsers.values()) parser.delete()

  for (const [file, names] of calls) {
    for (const name of new Set([...names, ...(types.get(file) ?? [])])) {
      const owners = swift.get(name)
      if (!owners) {
        if (names.has(name))
          incomplete.set(file, `unresolved Swift call ${name}`)
        continue
      }
      if (
        owners.length !== 1 ||
        path.dirname(owners[0]!) !== path.dirname(file) ||
        shadows.get(file)?.has(name)
      ) {
        incomplete.set(file, `ambiguous Swift reference to ${name}`)
      } else if (owners[0] !== file) edges.get(file)!.add(owners[0]!)
    }
  }
  return { edges, incomplete }
}
