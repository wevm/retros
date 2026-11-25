/** @module Reads direct package declarations without building projects or executing manifests. */
import type { Node } from 'web-tree-sitter'
import type * as Syntax from './syntax.js'

/** Manifest names recognized by the dependency check. */
export const names = [
  'build.zig.zon',
  'Cargo.toml',
  'go.mod',
  'package.json',
  'Package.swift',
]

function go(text: string) {
  const found: string[] = []
  let requireBlock = false
  for (const line of text.split('\n')) {
    const [directive = '', comment] = line.split('//')
    const value = directive.trim()
    if (value === ')') requireBlock = false
    else if (/^require\s*\($/.test(value)) requireBlock = true
    else if (!/^\s*indirect\b/.test(comment ?? '')) {
      const entry = requireBlock ? value : /^require\s+(.*)$/.exec(value)?.[1]
      const name = entry?.match(/^("[^"]+"|\S+)\s+v\S+/)?.[1]
      if (name) found.push(name.replace(/^"|"$/g, ''))
    }
  }
  return found
}

function swiftArgs(call: Node) {
  return (
    call.lastNamedChild?.firstNamedChild?.namedChildren.filter(
      (node) => node?.type === 'value_argument',
    ) ?? []
  )
}

function swift(root: Node) {
  const calls = root.descendantsOfType('call_expression')
  const packages = calls.filter(
    (node) => node?.firstNamedChild?.text === 'Package',
  )
  const [packageCall] = packages
  if (root.hasError || !packageCall || packages.length !== 1) return undefined
  // A manifest can change its dependency array after constructing the package. Do not
  // certify the initializer as the final value or execute the manifest to find out.
  if (
    root
      .descendantsOfType('navigation_suffix')
      .some(
        (node) => node?.childForFieldName('suffix')?.text === 'dependencies',
      )
  )
    return undefined
  const dependencies = swiftArgs(packageCall)
    .find((arg) => arg?.childForFieldName('name')?.text === 'dependencies')
    ?.childForFieldName('value')
  if (!dependencies) return []
  if (dependencies.type !== 'array_literal') return undefined

  const names: string[] = []
  for (const entry of dependencies.namedChildren) {
    if (!entry || entry.type.endsWith('comment')) continue
    if (
      entry.type !== 'call_expression' ||
      entry.firstNamedChild?.text !== '.package'
    )
      return undefined
    const origin = swiftArgs(entry)
      .find((arg) =>
        ['url', 'path', 'id'].includes(
          arg?.childForFieldName('name')?.text ?? '',
        ),
      )
      ?.childForFieldName('value')
    names.push(origin?.text.replace(/^"|"$/g, '') ?? entry.text)
  }
  return names
}

function zig(root: Node) {
  if (root.hasError) return undefined
  const fields = root.descendantsOfType('initializer_list')[0]?.namedChildren
  if (!fields) return undefined
  const deps = fields.find(
    (field) => field?.childForFieldName('left')?.text === '.dependencies',
  )
  if (!deps) return []
  const entries = deps.childForFieldName('right')?.firstNamedChild
  if (entries?.type !== 'initializer_list') return undefined
  return entries.namedChildren.flatMap((entry) => {
    const key = entry?.childForFieldName('left')?.childForFieldName('member')
    return key ? [key.text] : []
  })
}

/** Returns direct dependency names, or undefined for manifests that require evaluation. */
export async function dependencies(
  name: string,
  text: string,
  read: Awaited<ReturnType<typeof Syntax.create>>,
) {
  switch (name) {
    case 'Cargo.toml': {
      type Table = { dependencies?: object }
      const manifest = Bun.TOML.parse(text) as Table & {
        target?: Record<string, Table>
      }
      return [manifest, ...Object.values(manifest.target ?? {})].flatMap(
        (table) => Object.keys(table.dependencies ?? {}),
      )
    }
    case 'go.mod':
      return go(text)
    case 'package.json': {
      const manifest = JSON.parse(text) as Record<string, object | undefined>
      return [
        'dependencies',
        'optionalDependencies',
        'peerDependencies',
      ].flatMap((field) => Object.keys(manifest[field] ?? {}))
    }
    case 'Package.swift':
      return read('swift', text, swift)
    // ZON is a Zig expression, not a complete Zig source file.
    case 'build.zig.zon':
      return read('zig', `const manifest = ${text};`, zig)
    default:
      return undefined
  }
}
