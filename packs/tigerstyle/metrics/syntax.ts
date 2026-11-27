/** @module Reads function bodies, calls, assertions, and source locations from six Tree-sitter grammars. */
import { Language, type Node, Parser } from 'web-tree-sitter'

/** The grammar for each supported source extension. TSX has its own grammar. */
export const extensions = {
  '.c': 'c',
  '.cts': 'typescript',
  '.go': 'go',
  '.h': 'c',
  '.mts': 'typescript',
  '.rs': 'rust',
  '.swift': 'swift',
  '.ts': 'typescript',
  '.tsx': 'tsx',
  '.zig': 'zig',
} as const

/** A grammar in the pack's pinned WASM distribution. */
export type Grammar = (typeof extensions)[keyof typeof extensions]

/** Loads only the grammars used by the project. Syntax nodes do not escape the read callback. */
export async function create() {
  await Parser.init()
  const parsers = new Map<Grammar, Parser>()

  return async function read<T>(
    grammar: Grammar,
    options: create.read.Options<T>,
  ) {
    let parser = parsers.get(grammar)
    if (!parser) {
      // The bundled Rust grammar predates exclusive range patterns in stable Rust.
      const wasm = import.meta.resolve(
        grammar === 'rust'
          ? 'tree-sitter-rust/tree-sitter-rust.wasm'
          : `tree-sitter-wasms/out/tree-sitter-${grammar}.wasm`,
      )
      parser = new Parser().setLanguage(
        await Language.load(Bun.fileURLToPath(wasm)),
      )
      parsers.set(grammar, parser)
    }

    const tree = parser.parse(options.text)
    if (!tree) throw new Error(`Could not parse ${grammar} source`)
    try {
      return options.inspect(tree.rootNode)
    } finally {
      tree.delete()
    }
  }
}

/** The types of `create`. */
export declare namespace create {
  /** The types of the `read` function that `create` returns. */
  namespace read {
    /** The source text to parse, and the callback that reads its syntax tree. */
    type Options<T> = {
      /** Reads the syntax tree. Nodes do not remain valid after it returns. */
      inspect: (root: Node) => T
      /** The source text. */
      text: string
    }
  }
}

type Fn = {
  assertions: number
  calls: number[]
  end: number
  line: number
  name: string
  production: boolean
}

/** The function that encloses a node, whether the node is production code, and its self-call names. */
type Scope = {
  fn?: Fn | undefined
  production: boolean
  self?: Set<string> | undefined
}

const functionTypes = new Set([
  'arrow_function',
  'closure_expression',
  'deinit_declaration',
  'func_literal',
  'function',
  'function_declaration',
  'function_definition',
  'function_expression',
  'function_item',
  'generator_function',
  'generator_function_declaration',
  'init_declaration',
  'lambda_literal',
  'method_declaration',
  'method_definition',
])

const assertions: Record<Grammar, RegExp> = {
  c: /^assert$/,
  go: /^(?:assert\w*|assert\.\w+)$/,
  rust: /^(?:(?:std|core)::)?(?:debug_)?assert(?:_eq|_ne)?!$/,
  swift: /^(?:Swift\.)?(?:assert|precondition)$/,
  tsx: /^(?:assert(?:\.\w+)?|console\.assert|invariant)$/,
  typescript: /^(?:assert(?:\.\w+)?|console\.assert|invariant)$/,
  zig: /^(?:std\.debug\.)?assert$/,
}

function declarator(node: Node | null): Node | null {
  while (node && node.type !== 'identifier') {
    node = node.childForFieldName('declarator') ?? node.firstNamedChild
  }
  return node
}

function testAttribute(node: Node) {
  for (
    let prev = node.previousNamedSibling;
    prev;
    prev = prev.previousNamedSibling
  ) {
    if (prev.type.endsWith('comment')) continue
    if (prev.type !== 'attribute_item') break
    if (/^#\[\s*(?:test|cfg\(\s*test\s*\))\s*\]$/.test(prev.text)) return true
  }
  return false
}

function signature(node: Node, grammar: Grammar) {
  const c = node.type === 'function_definition'
  const name = (() => {
    if (c) return declarator(node.childForFieldName('declarator'))?.text
    if (node.type === 'init_declaration') return 'init'
    if (node.type === 'deinit_declaration') return 'deinit'
    return node.childForFieldName('name')?.text
  })()
  const parent = node.parent
  const binding =
    parent?.childForFieldName('value')?.id === node.id
      ? (parent.childForFieldName('name')?.text ??
        parent.childForFieldName('key')?.text)
      : undefined
  const receiver = node
    .childForFieldName('receiver')
    ?.firstNamedChild?.childForFieldName('name')?.text
  const calls = new Set<string>()
  for (const id of [name, binding]) {
    if (!id) continue
    if (receiver) calls.add(`${receiver}.${id}`)
    else if (node.type === 'method_definition') {
      calls.add(`this.${id}`)
      const owner = node.parent?.parent?.childForFieldName('name')?.text
      if (owner && node.children.some((child) => child?.type === 'static'))
        calls.add(`${owner}.${id}`)
    } else if (
      grammar === 'rust' &&
      node.parent?.type === 'declaration_list' &&
      ['impl_item', 'trait_item'].includes(node.parent.parent?.type ?? '')
    ) {
      calls.add(`Self::${id}`)
      if (
        node
          .childForFieldName('parameters')
          ?.descendantsOfType('self_parameter').length
      )
        calls.add(`self.${id}`)
    } else {
      calls.add(id)
      if (grammar === 'swift') {
        calls.add(`Self.${id}`)
        calls.add(`self.${id}`)
      }
    }
  }

  // A parameter can hide the function name, including C's nested pointer declarators.
  const head = c
    ? node.childForFieldName('declarator')
    : node.childForFieldName('parameters')
  for (const param of head?.descendantsOfType([
    'parameter_declaration',
    'parameter',
    'required_parameter',
    'optional_parameter',
  ]) ?? []) {
    if (!param) continue
    const id = c
      ? declarator(param.childForFieldName('declarator'))
      : (param.childForFieldName('pattern') ?? param.childForFieldName('name'))
    if (id) calls.delete(id.text)
  }
  const single = node.childForFieldName('parameter')
  if (single) calls.delete(single.text)
  if (grammar === 'swift')
    for (const param of node.namedChildren) {
      if (param?.type === 'parameter')
        calls.delete(param.childForFieldName('name')?.text ?? '')
    }
  return { calls, name: name ?? binding }
}

// A declaration hides a bare self-call (or its receiver) only in its own lexical block.
function localBindings(node: Node): readonly string[] {
  if (
    node.type === 'lexical_declaration' ||
    node.type === 'variable_declaration'
  ) {
    return node.namedChildren.flatMap(
      (child) => child?.childForFieldName('name')?.text ?? [],
    )
  }
  if (node.type === 'declaration')
    return node
      .childrenForFieldName('declarator')
      .flatMap((child) => declarator(child)?.text ?? [])
  if (node.type === 'let_declaration')
    return [node.childForFieldName('pattern')?.text ?? '']
  if (node.type === 'short_var_declaration')
    return (
      node
        .childForFieldName('left')
        ?.namedChildren.flatMap((child) => child?.text ?? []) ?? []
    )
  if (node.type === 'property_declaration')
    return [node.childForFieldName('name')?.text ?? '']
  return []
}

function call(node: Node) {
  let callee = node.childForFieldName('function') ?? node.firstNamedChild
  if (callee?.type === 'generic_function')
    callee = callee.childForFieldName('function')
  if (!callee) return undefined
  const name = callee.text.replace(/\s+/g, '')
  const argumentsNode =
    node.childForFieldName('arguments') ?? node.lastNamedChild?.firstNamedChild
  const argument =
    argumentsNode?.type === 'value_arguments'
      ? argumentsNode.firstNamedChild?.childForFieldName('value')
      : (node
          .childForFieldName('arguments')
          ?.namedChildren.find(
            (child) => child && !child.type.endsWith('comment'),
          ) ??
        node.namedChildren.find(
          (child) =>
            child && child.id !== callee.id && !child.type.endsWith('comment'),
        ))
  return { argument, name }
}

function compound(node: Node, rustMacro = false): boolean {
  if (
    node.type.endsWith('comment') ||
    /string|char_literal|regex/.test(node.type)
  )
    return false
  const children = node.children
  for (const child of children) {
    if (!child) continue
    if (rustMacro && child.type === ',') break
    if (['&&', '||', 'and', 'or'].includes(child.type)) return true
    if (child.isNamed && compound(child)) return true
  }
  return false
}

/** Measures syntactic calls, not call graphs. Named function bodies form separate assertion scopes. */
export function measure(root: Node, options: measure.Options) {
  const { grammar, text } = options
  const functions: Fn[] = []
  const comments: { line: number; text: string }[] = []
  const compounds: string[] = []
  const sized: string[] = []
  const code = text.split('')
  let error: string | undefined

  function visit(node: Node, scope: Scope) {
    const { fn, self } = scope
    let production = scope.production
    const type = node.type
    const line = node.startPosition.row + 1
    if (!error && (node.isError || node.isMissing))
      error = `grammar could not parse source near line ${line}`
    if (
      type.endsWith('comment') ||
      /^(?:.*_)?string(?:_literal)?$|^multi_line_str_text$|^template_chars$/.test(
        type,
      )
    ) {
      for (let at = node.startIndex; at < node.endIndex; at++)
        if (code[at] !== '\n') code[at] = ' '
      if (type.endsWith('comment')) {
        const value = node.text
        if (
          /^\/\/(?![!/])/.test(value) &&
          !text
            .slice(
              text.lastIndexOf('\n', node.startIndex - 1) + 1,
              node.startIndex,
            )
            .trim()
        ) {
          comments.push({ line, text: value.slice(2).trim() })
        }
        return
      }
      // Interpolation inside a literal still contains executable syntax.
    }

    if (grammar === 'c' && (type === 'preproc_if' || type === 'preproc_elif')) {
      const condition = node.childForFieldName('condition')?.text
      const other = node.childForFieldName('alternative')
      if (condition === '0' || condition === '1') {
        for (const child of node.namedChildren) {
          if (child && (child.id === other?.id) === (condition === '0'))
            visit(child, { fn, production, self })
        }
        return
      }
    }
    if (
      type === 'test_declaration' ||
      (grammar === 'rust' &&
        ['function_item', 'mod_item'].includes(type) &&
        testAttribute(node))
    )
      production = false

    if (functionTypes.has(type)) {
      const body =
        node.childForFieldName('body') ??
        node.namedChildren.find(
          (child) => child && ['block', 'statements'].includes(child.type),
        )
      if (body) {
        const sig = signature(node, grammar)
        const current: Fn = {
          assertions: 0,
          calls: [],
          end: node.endPosition.row + 1,
          line,
          name: sig.name ?? '<anonymous>',
          production: production && !!sig.name,
        }
        functions.push(current)
        for (const child of node.namedChildren)
          if (child)
            visit(child, {
              fn: child.id === body.id ? current : undefined,
              production,
              self: child.id === body.id ? sig.calls : undefined,
            })
        return
      }
    }

    if (type === 'call_expression') {
      const site = call(node)
      if (site && self?.has(site.name)) fn?.calls.push(line)
      if (production && site && assertions[grammar].test(site.name)) {
        if (fn) fn.assertions++
        if (site.argument && compound(site.argument))
          compounds.push(
            `line ${line}: ${fn?.name ?? '<file>'}: ${node.text.slice(0, 80)}`,
          )
      }
    } else if (
      production &&
      grammar === 'rust' &&
      type === 'macro_invocation'
    ) {
      const name = node.childForFieldName('macro')?.text
      const args = node.lastNamedChild
      if (name && assertions.rust.test(`${name}!`)) {
        if (fn) fn.assertions++
        if (args && compound(args, true))
          compounds.push(
            `line ${line}: ${fn?.name ?? '<file>'}: ${node.text.slice(0, 80)}`,
          )
      }
    }

    if (
      ['primitive_type', 'builtin_type'].includes(type) &&
      /^[ui]size$/.test(node.text)
    )
      sized.push(`line ${line}: ${node.text}`)
    let active = self
    for (const child of node.namedChildren) {
      if (!child) continue
      visit(child, { fn, production, self: active })
      if (active?.size) {
        const bindings = localBindings(child)
        if (bindings.length)
          active = new Set(
            [...active].filter(
              (name) => !bindings.includes(name.split(/[.:]/)[0]!),
            ),
          )
      }
    }
  }

  visit(root, { production: true })
  if (!error && root.hasError)
    error = 'grammar could not parse the complete source'
  return { code: code.join(''), comments, compounds, error, functions, sized }
}

/** The types of `measure`. */
export declare namespace measure {
  /** The grammar of the source, and its text. */
  type Options = {
    /** The grammar that parsed `root`. */
    grammar: Grammar
    /** The source text, for comment context and blanked code. */
    text: string
  }
}
