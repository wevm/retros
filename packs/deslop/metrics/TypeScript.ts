/** @module Resolves TypeScript imports, calls and field receivers to their declarations. */
import * as path from 'node:path'
import * as ts from 'typescript'

/** Reads maintained files as one program. An unresolved reference is not a clean finding. */
export function analyze(files: readonly string[]) {
  const edges = new Map(files.map((file) => [file, new Set<string>()]))
  const fieldsOf = new Map<string, Set<string>>()
  const shown = new Map(files.map((file) => [file, new Set<string>()]))
  const properties = new Map<string, Map<number, string | undefined>>()
  let incomplete: string | undefined
  let unknownCall: string | undefined
  if (files.length === 0)
    return { edges, fieldsOf, incomplete, properties, shown, unknownCall }

  const program = ts.createProgram([...files], {
    jsx: ts.JsxEmit.Preserve,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    noEmit: true,
    skipLibCheck: true,
    strict: true,
    target: ts.ScriptTarget.ESNext,
  })
  const checker = program.getTypeChecker()
  for (const file of files) {
    const tree = program.getSourceFile(file)
    if (!tree || program.getSyntacticDiagnostics(tree).length > 0) {
      incomplete ??= `TypeScript could not parse ${file}`
      continue
    }
    const fields = new Map<number, string | undefined>()
    properties.set(file, fields)

    function visit(node: ts.Node) {
      if (ts.isPropertyDeclaration(node) || ts.isPropertySignature(node)) {
        const parent = node.parent
        const name =
          ts.isTypeLiteralNode(parent) &&
          ts.isTypeAliasDeclaration(parent.parent)
            ? parent.parent.name
            : ts.getNameOfDeclaration(parent)
        if (
          name &&
          ts.isIdentifier(name) &&
          (ts.isIdentifier(node.name) || ts.isPrivateIdentifier(node.name))
        ) {
          const key = `${file}:${name.text}`
          if (!fieldsOf.has(key)) fieldsOf.set(key, new Set())
          fieldsOf.get(key)!.add(node.name.text)
        }
      }
      const specifier = (() => {
        if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node))
          return node.moduleSpecifier
        if (
          ts.isCallExpression(node) &&
          node.expression.kind === ts.SyntaxKind.ImportKeyword
        )
          return node.arguments[0]
        if (ts.isExternalModuleReference(node)) return node.expression
        return undefined
      })()
      if (specifier) {
        const location = `${file}:${tree!.getLineAndCharacterOfPosition(specifier.getStart()).line + 1}`
        if (!ts.isStringLiteralLike(specifier))
          incomplete ??= `nonliteral module reference at ${location}`
        else {
          const symbol = checker.getSymbolAtLocation(specifier)
          const target = symbol?.declarations?.find(ts.isSourceFile)
          if (!target)
            incomplete ??= `unresolved module ${specifier.text} at ${location}`
          else {
            const to = path.relative('.', target.fileName)
            if (edges.has(to) && file !== to) edges.get(file)!.add(to)
          }
        }
      }

      if (
        ts.isCallExpression(node) &&
        node.expression.kind !== ts.SyntaxKind.ImportKeyword
      ) {
        const definition = checker.getResolvedSignature(node)?.declaration
        if (!definition) unknownCall ??= `unresolved call in ${file}`
        else {
          const to = path.relative('.', definition.getSourceFile().fileName)
          const name =
            (ts.isArrowFunction(definition) ||
              ts.isFunctionExpression(definition)) &&
            ts.isVariableDeclaration(definition.parent)
              ? definition.parent.name
              : ts.getNameOfDeclaration(definition)
          if (to !== file && name && ts.isIdentifier(name))
            shown.get(to)?.add(name.text)
        }
      }

      if (ts.isPropertyAccessExpression(node)) {
        const symbol = checker.getSymbolAtLocation(node.name)
        const declarations = symbol?.declarations
        const owners = new Set(
          declarations?.map((declaration) => {
            const type = declaration.parent
            if (
              (ts.isClassDeclaration(type) ||
                ts.isInterfaceDeclaration(type)) &&
              type.name
            )
              return `${path.relative('.', type.getSourceFile().fileName)}:${type.name.text}`
            if (
              ts.isTypeLiteralNode(type) &&
              ts.isTypeAliasDeclaration(type.parent)
            )
              return `${path.relative('.', type.getSourceFile().fileName)}:${type.parent.name.text}`
            // A resolved object literal or library member does not belong to a project class.
            return ''
          }),
        )
        fields.set(
          node.name.getStart(),
          owners.size === 1 ? [...owners][0] : undefined,
        )
      }
      ts.forEachChild(node, visit)
    }
    visit(tree)
  }
  return { edges, fieldsOf, incomplete, properties, shown, unknownCall }
}
