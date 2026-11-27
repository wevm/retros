/** @module Measures Rust types, imports, attributes, comments, and macro arguments on the Rust syntax tree. */
import * as url from 'node:url'
import { Language, Parser, type Node } from 'web-tree-sitter'

/** Loads the pack's pinned Rust grammar. No build or install script runs in the measured project. */
export async function parser(): Promise<Parser> {
  await Parser.init()
  const language = await Language.load(
    url.fileURLToPath(
      import.meta.resolve('tree-sitter-rust/tree-sitter-rust.wasm'),
    ),
  )
  return new Parser().setLanguage(language)
}

function children(node: Node | null | undefined): readonly Node[] {
  return (
    node?.namedChildren.filter(
      (child): child is Node => child !== null && !child.isExtra,
    ) ?? []
  )
}

function identifier(node: Node | null | undefined): string {
  if (!node) return ''
  const name = node.childForFieldName('name')
  if (name)
    return [identifier(node.childForFieldName('path')), identifier(name)]
      .filter(Boolean)
      .join('::')
  return node.text.replace(/^r#/, '')
}

/** Collects the explicit `use` bindings in their lexical scopes, including brace groups. */
function imports(root: Node) {
  const scopes = new Map<number, Map<string, string>>()
  for (const declaration of root.descendantsOfType('use_declaration')) {
    const scope = declaration?.parent?.id
    const argument = declaration?.childForFieldName('argument')
    if (!scope || !argument) continue
    const bindings = scopes.get(scope) ?? new Map<string, string>()
    scopes.set(scope, bindings)

    function collect(node: Node, prefix: string): void {
      if (node.type === 'use_list') {
        for (const child of children(node)) collect(child, prefix)
        return
      }
      const from = node.childForFieldName('path')
      const list = node.childForFieldName('list')
      if (from && list) {
        collect(list, `${prefix}${identifier(from)}::`)
        return
      }
      const target = node.type === 'use_as_clause' ? from : node
      const full = `${prefix}${identifier(target)}`.replace(/::self$/, '')
      const name =
        node.childForFieldName('alias')?.text ?? full.split('::').at(-1)
      if (name) bindings.set(name, full)
    }

    collect(argument, '')
  }
  return (node: Node | null | undefined) => {
    const value = identifier(node)
    const [first, ...tail] = value.split('::')
    for (let scope = node?.parent; scope; scope = scope.parent) {
      const binding = scopes.get(scope.id)?.get(first ?? '')
      if (binding) return [binding, ...tail].join('::')
    }
    return value
  }
}

function macroArguments(tokens: Node): readonly (readonly Node[])[] {
  const groups: (readonly Node[])[] = []
  let group: Node[] = []
  // Nested delimiters and literals are each one token-tree child, not text to split.
  for (const token of tokens.children.slice(1, -1)) {
    if (!token || token.isExtra) continue
    if (token.type === ',') {
      groups.push(group)
      group = []
    } else group.push(token)
  }
  groups.push(group)
  return groups
}

/** Parses the assertion's first expression, not its optional formatting arguments. */
function assertion(parser: Parser, tokens: Node) {
  const tree = parser.parse(`fn audit(){check(${tokens.text.slice(1, -1)});}`)
  try {
    const args = tree?.rootNode.descendantsOfType('arguments')[0]
    let condition = children(args)[0]
    while (condition?.type === 'parenthesized_expression')
      condition = children(condition)[0]
    const operator = condition?.childForFieldName('operator')?.type
    const fn = condition?.childForFieldName('function')
    return {
      compound: operator === '&&' || operator === '||',
      weak:
        fn?.type === 'field_expression' &&
        fn.childForFieldName('field')?.text === 'is_err' &&
        children(condition?.childForFieldName('arguments')).length === 0,
    }
  } finally {
    tree?.delete()
  }
}

/** Returns distinct source findings and an explicit reason when the grammar cannot parse the file. */
export function inspect(parser: Parser, source: string) {
  const problems = new Map<string, readonly string[]>()
  const tree = parser.parse(source)
  try {
    const root = tree?.rootNode
    if (!root || root.hasError)
      return { failure: 'the Rust grammar could not parse this file', problems }
    const resolve = imports(root)
    const first = root.namedChildren.find(
      (node) =>
        node && node.type !== 'inner_attribute_item' && node.type !== 'shebang',
    )
    if (first?.type !== 'line_comment' || !first.childForFieldName('inner'))
      problems.set('module-docs', [
        'the file does not start with a `//!` comment',
      ])

    let assertions = 0
    let inline = 0
    for (const node of root.descendantsOfType([
      'attribute_item',
      'call_expression',
      'generic_type',
      'line_comment',
      'macro_invocation',
      'tuple_type',
      'use_as_clause',
    ])) {
      if (!node) continue
      const add = (metric: string, text: string) => {
        problems.set(metric, [
          ...(problems.get(metric) ?? []),
          `line ${node.startPosition.row + 1}: ${text}`,
        ])
      }
      if (
        node.type === 'line_comment' &&
        /^\/{2,}\s*([-=*#~_])\1{2,}/.test(node.text)
      )
        add('comment-dividers', 'a decorative divider')
      if (
        node.type === 'use_as_clause' &&
        node.childForFieldName('alias')?.text !== '_'
      )
        add('import-renames', 'a renamed import')
      if (node.type === 'tuple_type' && children(node).length >= 3)
        add('wide-tuples', 'a tuple of three or more types')

      if (node.type === 'attribute_item') {
        const attribute = node.firstNamedChild
        const name = attribute?.firstNamedChild?.text
        const args = children(attribute?.childForFieldName('arguments'))
        if (
          name === 'test' ||
          (name === 'cfg' && args.length === 1 && args[0]?.text === 'test')
        )
          add(
            'unit-tests',
            `#[${name === 'cfg' ? 'cfg(test)' : 'test'}] outside tests/`,
          )
      }
      if (node.type === 'generic_type') {
        const name = resolve(node.childForFieldName('type')).split('::').at(-1)
        const [first, error] = children(
          node.childForFieldName('type_arguments'),
        )
        if (
          name === 'Rc' &&
          first?.type === 'generic_type' &&
          resolve(first.childForFieldName('type')).split('::').at(-1) ===
            'RefCell'
        )
          add('shared-mutability', 'Rc<RefCell<…>>')
        if (name === 'Result' && error) {
          let inner = children(error.childForFieldName('type_arguments'))[0]
          while (inner?.type === 'bounded_type') inner = children(inner)[0]
          const boxed =
            error.type === 'generic_type' &&
            resolve(error.childForFieldName('type')).split('::').at(-1) ===
              'Box' &&
            inner?.type === 'dynamic_type' &&
            resolve(inner.childForFieldName('trait')).split('::').at(-1) ===
              'Error'
          if (
            boxed ||
            resolve(error).split('::').at(-1) === 'String' ||
            (error.type === 'reference_type' &&
              error.childForFieldName('type')?.text === 'str')
          )
            add('string-errors', 'a string as the error')
        }
      }
      if (node.type === 'call_expression') {
        let fn = node.childForFieldName('function')
        if (fn?.type === 'generic_function')
          fn = fn.childForFieldName('function')
        if (fn?.type !== 'scoped_identifier' && fn?.type !== 'identifier')
          continue
        const name = resolve(fn)
        const args = children(node.childForFieldName('arguments'))
        if (/(?:^|::)(?:thread|time)::sleep$/.test(name))
          add('test-sleeps', 'a test that sleeps a fixed time')
        if (
          !args.length &&
          /(?:^|::)(?:unbounded_channel|mpsc::channel|(?:crossbeam_channel|flume)::unbounded)$/.test(
            name,
          )
        )
          add('unbounded-channels', 'a channel without a capacity')
        if (/(?:^|::)Duration::from_(?:secs|millis|micros|nanos)$/.test(name)) {
          const arg = args[0]
          const variable =
            arg?.type === 'identifier'
              ? arg.text
              : arg?.childForFieldName('field')?.text
          if (
            variable &&
            !/(secs|seconds|ms|millis|us|micros|ns|nanos)$/.test(variable)
          )
            add('duration-names', 'a duration from a name without its unit')
        }
      }
      if (node.type !== 'macro_invocation') continue
      const name =
        resolve(node.childForFieldName('macro')).split('::').at(-1) ?? ''
      const tokens = node.lastNamedChild
      if (tokens?.type !== 'token_tree') continue
      const args = macroArguments(tokens)
      if (/^(?:(?:debug_)?assert(?:_eq|_ne)?|assert_\w*snapshot)$/.test(name)) {
        assertions++
        if (
          name.endsWith('snapshot') &&
          args.some(
            (arg) =>
              arg[0]?.type === '@' &&
              ['string_literal', 'raw_string_literal'].includes(
                arg[1]?.type ?? '',
              ),
          )
        )
          inline++
      }
      if (name === 'assert' || name === 'debug_assert') {
        const checked = assertion(parser, tokens)
        if (checked.compound)
          add('compound-asserts', 'one assertion checks two conditions')
        if (checked.weak)
          add(
            'weak-error-assertions',
            'an assertion that only checks for an error',
          )
      }
      if (['trace', 'debug', 'info', 'warn', 'error'].includes(name)) {
        const message = args.find(
          (arg) =>
            arg.length === 1 &&
            ['string_literal', 'raw_string_literal'].includes(
              arg[0]?.type ?? '',
            ),
        )?.[0]
        if (
          message &&
          /\{[^}]*\}/.test(
            message.text.replaceAll('{{', '').replaceAll('}}', ''),
          )
        )
          add(
            'log-interpolation',
            'a log message that interpolates values, not fields',
          )
      }
    }
    if (inline < assertions)
      problems.set('inline-snapshots', [
        `${assertions - inline} of ${assertions} assertions are not inline snapshots`,
      ])
    return { failure: undefined, problems }
  } finally {
    tree?.delete()
  }
}
