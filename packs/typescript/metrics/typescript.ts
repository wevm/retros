/**
 * Measures the project's TypeScript against its conventions. oxlint reports each convention that
 * one of its rules covers, from one run with only those rules. The script reads the rest from
 * oxc-parser's syntax tree, such as import styles, error names, and TSDoc, and reads the manifests
 * and compiler options as JSON. Each source file or manifest is an item, and a failing check
 * carries the first problem as its note.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as module from 'node:module'
import * as os from 'node:os'
import * as path from 'node:path'

import { parseSync } from 'oxc-parser'

/**
 * Paths that are not maintained source, such as build output, installed packages, and vendored
 * packs.
 */
const ignored = [
  /(^|\/)(\.agents\/packs|dist|build|coverage|node_modules|target)\//,
  /\.d\.[cm]?ts$/,
]

/** Compiler options the conventions treat as design constraints. */
const compilerOptions = [
  'exactOptionalPropertyTypes',
  'noUncheckedIndexedAccess',
  'strict',
  'verbatimModuleSyntax',
]

/** Base classes that frameworks require, which the class ban allows. */
const frameworkClasses =
  /^(Component|DurableObject|HTMLElement|LitElement|PureComponent|WorkerEntrypoint|WorkflowEntrypoint)$/

/** Calls and packages that replace real behavior in tests. */
const mockCalls = [
  'doMock',
  'fn',
  'hoisted',
  'mock',
  'mocked',
  'spyOn',
  'stubEnv',
  'stubGlobal',
  'useFakeTimers',
]
const mockPackages = [
  'jest-mock',
  'msw',
  'msw/node',
  'nock',
  'sinon',
  'testdouble',
]

/** Properties that read the clock or sleep, which the conventions ban in tests. */
const sleepProperties = [
  {
    message: 'Wait for an observable condition.',
    object: 'Bun',
    property: 'sleep',
  },
  { message: 'Wait for an observable condition.', property: 'waitForTimeout' },
]
const randomProperty = {
  message: 'Use crypto.getRandomValues().',
  object: 'Math',
  property: 'random',
}

/**
 * The oxlint rules behind each lint metric. The script runs oxlint once with only these rules, so
 * the project's own oxlint configuration does not change what the metrics read.
 */
const oxlint = {
  categories: {},
  overrides: [
    {
      files: ['**/src/**', 'src/**'],
      rules: { 'eslint/no-console': 'error' },
    },
    {
      files: ['**/*.test.*', '**/*.spec.*', '**/*.test-d.*'],
      rules: {
        'eslint/no-console': 'off',
        'eslint/no-restricted-globals': ['error', 'setTimeout', 'setInterval'],
        'eslint/no-restricted-imports': [
          'error',
          {
            paths: [
              ...mockPackages.map((name) => ({ name })),
              { importNames: ['mock', 'spyOn'], name: 'bun:test' },
            ],
          },
        ],
        'eslint/no-restricted-properties': [
          'error',
          randomProperty,
          ...sleepProperties,
        ],
        'jest/no-restricted-jest-methods': [
          'error',
          Object.fromEntries(mockCalls.map((name) => [name, null])),
        ],
        'vitest/no-restricted-vi-methods': [
          'error',
          Object.fromEntries(mockCalls.map((name) => [name, null])),
        ],
      },
    },
  ],
  plugins: ['import', 'jest', 'typescript', 'vitest'],
  rules: {
    'eslint/max-params': ['error', { max: 2 }],
    'eslint/no-nested-ternary': 'error',
    'eslint/no-restricted-properties': ['error', randomProperty],
    'import/extensions': ['error', 'always', { ignorePackages: true }],
    'import/no-cycle': 'error',
    'import/no-unassigned-import': [
      'error',
      { allow: ['**/*.css', '**/*.scss'] },
    ],
    'typescript/consistent-type-definitions': ['error', 'type'],
    'typescript/consistent-type-exports': 'error',
    'typescript/consistent-type-imports': 'error',
    'typescript/no-explicit-any': 'error',
  },
}

/** The lint metrics, each with the rules it reads and the files it covers. */
const rules: Record<
  string,
  { files: () => readonly string[]; match: (diagnostic: Diagnostic) => boolean }
> = {
  'console-calls': {
    files: () => production(),
    match: (diagnostic) => diagnostic.rule === 'eslint(no-console)',
  },
  'explicit-any': {
    files: () => files,
    match: (diagnostic) => diagnostic.rule === 'typescript(no-explicit-any)',
  },
  'import-cycles': {
    files: () => files,
    match: (diagnostic) => diagnostic.rule === 'import(no-cycle)',
  },
  'import-extensions': {
    files: () => files,
    match: (diagnostic) => diagnostic.rule === 'import(extensions)',
  },
  interfaces: {
    files: () => files,
    match: (diagnostic) =>
      diagnostic.rule === 'typescript(consistent-type-definitions)',
  },
  'math-random': {
    files: () => files,
    match: (diagnostic) =>
      diagnostic.rule === 'eslint(no-restricted-properties)' &&
      diagnostic.message.includes('Math.random'),
  },
  mocks: {
    files: () => tests,
    match: (diagnostic) =>
      /no-restricted-(imports|jest-methods|vi-methods)\)$/.test(
        diagnostic.rule,
      ),
  },
  'nested-ternaries': {
    files: () => files,
    match: (diagnostic) => diagnostic.rule === 'eslint(no-nested-ternary)',
  },
  'positional-parameters': {
    files: () => files,
    match: (diagnostic) => diagnostic.rule === 'eslint(max-params)',
  },
  'side-effect-imports': {
    files: () => files,
    match: (diagnostic) => diagnostic.rule === 'import(no-unassigned-import)',
  },
  sleeps: {
    files: () => tests,
    match: (diagnostic) =>
      diagnostic.rule === 'eslint(no-restricted-globals)' ||
      (diagnostic.rule === 'eslint(no-restricted-properties)' &&
        !diagnostic.message.includes('Math.random')),
  },
  'type-imports': {
    files: () => files,
    match: (diagnostic) =>
      /consistent-type-(imports|exports)\)$/.test(diagnostic.rule),
  },
}

const files = list(['*.ts', '*.tsx', '*.mts', '*.cts']).filter(
  (file) => !ignored.some((pattern) => pattern.test(file)),
)
const tests = files.filter((file) =>
  /\.(test|spec|test-d)\.[cm]?tsx?$/.test(file),
)
const sources = new Map(files.map((file) => [file, read(file)]))
const lines: string[] = []
/** The diagnostics of the oxlint run with the pack's rules, once it ran. */
const cache: { configured?: readonly Diagnostic[] } = {}

if (files.length === 0) {
  process.stderr.write('Found no TypeScript files to measure\n')
  process.exit(1)
}

types()
modules()
style()
docs()
testing()
tooling()
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Writes type errors, compiler options, and the type constructs the conventions ban. */
function types(): void {
  const compiler = ['node_modules/.bin/tsgo', 'node_modules/.bin/tsc'].find(
    (bin) => fs.existsSync(bin),
  )
  if (compiler) {
    const output = Bun.spawnSync([
      compiler,
      '--noEmit',
      '--pretty',
      'false',
    ]).stdout.toString()
    const errors = [
      ...output.matchAll(/^(.+?)\((\d+),\d+\): error (TS\d+: .+)$/gm),
    ]
    for (const file of files) {
      const error = errors.find((match) => path.normalize(match[1]!) === file)
      check({
        item: file,
        metric: 'type-errors',
        problems: error ? [`line ${error[2]}: ${error[3]}`] : [],
      })
    }
  } else
    lines.push('type-errors skip note "no tsgo or tsc in node_modules/.bin"')

  const options = tsconfig('tsconfig.json')
  for (const option of compilerOptions)
    check({
      item: option,
      metric: 'compiler-options',
      problems:
        options?.[option] === true
          ? []
          : [
              options
                ? `tsconfig.json does not set ${option}`
                : 'no tsconfig.json',
            ],
    })

  lint(['explicit-any', 'interfaces'])

  // oxlint has no rule that bans enums or requires `readonly` arrays, so the syntax tree answers.
  for (const [file, source] of sources) {
    const enums: string[] = []
    const arrays: string[] = []
    walk(source, (node, parents) => {
      if (node.type === 'TSEnumDeclaration')
        enums.push(`line ${source.line(node)}: enum ${node.id.name}`)
      const parent = parents.at(-1)
      const readonly =
        parent?.type === 'TSTypeOperator' && parent.operator === 'readonly'
      if (node.type === 'TSArrayType' && !readonly)
        arrays.push(`line ${source.line(node)}: ${source.text(node)}`)
      if (node.type === 'TSTypeReference' && node.typeName?.name === 'Array')
        arrays.push(`line ${source.line(node)}: ${source.text(node)}`)
    })
    check({ item: file, metric: 'enums', problems: enums })
    check({ item: file, metric: 'mutable-arrays', problems: arrays })
  }
}

/**
 * Writes import extensions and styles, re-exports, dynamic and side-effect imports, and JavaScript
 * files.
 */
function modules(): void {
  const builtins = new Set(module.builtinModules)
  lint([
    'import-extensions',
    'type-imports',
    'side-effect-imports',
    'import-cycles',
  ])

  // oxlint has no rule for import styles, flattened re-exports, or dynamic imports.
  for (const [file, source] of sources) {
    const namespaces: string[] = []
    const node: string[] = []
    const reexports: string[] = []
    const dynamic: string[] = []
    walk(source, (item) => {
      const specifier: unknown = item.source?.value
      const relative =
        typeof specifier === 'string' && specifier.startsWith('.')

      if (item.type === 'ImportDeclaration' && item.importKind !== 'type') {
        const values = item.specifiers.filter(
          (specifier: Node) =>
            specifier.type === 'ImportSpecifier' &&
            specifier.importKind !== 'type',
        )
        const target = path.join(
          path.dirname(file),
          String(specifier).replace(/\.[cm]?jsx?$/, ''),
        )
        const component =
          file.endsWith('.tsx') || fs.existsSync(`${target}.tsx`)
        if (
          relative &&
          values.length > 0 &&
          /^[A-Z]/.test(path.basename(String(specifier))) &&
          !component
        )
          namespaces.push(
            `line ${source.line(item)}: named import from '${specifier}'`,
          )
        const name = String(specifier)
        if (
          values.length > 0 &&
          (name.startsWith('node:') ||
            (builtins.has(name) && !name.includes(':')))
        )
          node.push(`line ${source.line(item)}: named import from '${name}'`)
      }

      if (item.type === 'ExportAllDeclaration' && !item.exported && relative)
        reexports.push(
          `line ${source.line(item)}: export * from '${specifier}'`,
        )
      if (
        item.type === 'ExportNamedDeclaration' &&
        relative &&
        item.exportKind !== 'type' &&
        item.specifiers.some(
          (specifier: Node) => specifier.exportKind !== 'type',
        )
      )
        reexports.push(
          `line ${source.line(item)}: export { … } from '${specifier}'`,
        )
      if (item.type === 'ImportExpression')
        dynamic.push(`line ${source.line(item)}: import(…)`)
    })
    check({ item: file, metric: 'namespace-imports', problems: namespaces })
    check({ item: file, metric: 'node-imports', problems: node })
    check({ item: file, metric: 'flat-reexports', problems: reexports })
    check({ item: file, metric: 'dynamic-imports', problems: dynamic })
  }

  const javascript = list(['*.js', '*.jsx', '*.mjs', '*.cjs']).filter(
    (file) =>
      !ignored.some((pattern) => pattern.test(file)) &&
      !/(^|\/)[^/]*\.config\.[cm]?js$/.test(file),
  )
  check({
    metric: 'javascript-files',
    problems: javascript.map((file) => `${file} is JavaScript`),
  })

  const packages = manifests().filter(
    (manifest) => manifest.json.exports || manifest.json.main,
  )
  for (const manifest of packages)
    check({
      item: manifest.file,
      metric: 'side-effects',
      problems:
        manifest.json.sideEffects === false ||
        Array.isArray(manifest.json.sideEffects)
          ? []
          : ['package.json does not set "sideEffects"'],
    })
  if (packages.length === 0)
    lines.push('side-effects skip note "no package.json with exports"')
}

/** Writes classes, error names, constants, parameters, ternaries, local types, and module state. */
function style(): void {
  for (const [file, source] of sources) {
    const problems: Record<string, string[]> = {}
    const add = (problem: Problem) =>
      (problems[problem.metric] ??= []).push(
        `line ${source.line(problem.node)}: ${problem.text}`,
      )
    const exported = new Set<string>()
    const types: Node[] = []

    walk(source, (node, parents) => {
      const parent = parents.at(-1)

      if (node.type === 'ClassDeclaration' || node.type === 'ClassExpression') {
        const base =
          node.superClass?.type === 'MemberExpression'
            ? node.superClass.property.name
            : node.superClass?.name
        const name = node.id?.name ?? 'anonymous class'
        if (
          !(base ?? '').endsWith('Error') &&
          !frameworkClasses.test(base ?? '')
        )
          add({
            metric: 'classes',
            node,
            text: `class ${name} is not an error`,
          })
        if ((base ?? '').endsWith('Error')) {
          let value: string | undefined
          walk({ ...source, program: node }, (member) => {
            if (
              member.type === 'PropertyDefinition' &&
              member.key?.name === 'name' &&
              typeof member.value?.value === 'string'
            )
              value = member.value.value
            if (
              member.type === 'AssignmentExpression' &&
              member.left?.object?.type === 'ThisExpression' &&
              member.left.property?.name === 'name' &&
              typeof member.right?.value === 'string'
            )
              value = member.right.value
          })
          if (!value || !/^[A-Z]\w*(\.[A-Z]\w*)+$/.test(value))
            add({
              metric: 'error-names',
              node,
              text: `${name} has name ${value ? `'${value}'` : 'unset'}, not 'Module.${name}'`,
            })
        }
      }

      const topLevel =
        parent?.type === 'Program' ||
        (parent?.type === 'ExportNamedDeclaration' &&
          parents.at(-2)?.type === 'Program')
      if (node.type === 'VariableDeclaration' && topLevel) {
        for (const declarator of node.declarations) {
          const name = declarator.id?.name ?? ''
          if (node.kind === 'const' && /^[A-Z][A-Z0-9_]+$/.test(name))
            add({ metric: 'uppercase-constants', node, text: `const ${name}` })
          if (node.kind !== 'const')
            add({
              metric: 'module-state',
              node,
              text: `${node.kind} ${name || 'binding'} at module scope`,
            })
          const constructed =
            declarator.init?.type === 'NewExpression'
              ? declarator.init.callee?.name
              : undefined
          if (
            node.kind === 'const' &&
            ['Map', 'Set', 'WeakMap', 'WeakSet'].includes(constructed) &&
            declarator.init.arguments.length === 0
          )
            add({
              metric: 'module-state',
              node,
              text: `new ${constructed}() at module scope`,
            })
          if (
            parent?.type === 'ExportNamedDeclaration' &&
            /Function|Arrow/.test(declarator.init?.type ?? '')
          )
            exported.add(name)
        }
      }
      if (
        node.type === 'FunctionDeclaration' &&
        parent?.type === 'ExportNamedDeclaration' &&
        node.id
      )
        exported.add(node.id.name)
      if (
        (node.type === 'TSTypeAliasDeclaration' ||
          node.type === 'TSInterfaceDeclaration') &&
        parent?.type === 'ExportNamedDeclaration' &&
        parents.at(-2)?.type === 'Program'
      )
        types.push(node)

      if (
        /^(FunctionDeclaration|FunctionExpression|ArrowFunctionExpression|TSDeclareFunction)$/.test(
          node.type,
        )
      ) {
        const params = node.params.filter(
          (param: Node) => param.name !== 'this',
        )
        const callback =
          (parent?.type === 'CallExpression' ||
            parent?.type === 'NewExpression') &&
          parent.arguments.includes(node)
        const test =
          callback &&
          /^(test|it)$/.test(
            parent.callee?.name ?? parent.callee?.object?.name ?? '',
          )
        const name =
          node.id?.name ??
          (parent?.type === 'VariableDeclarator'
            ? parent.id?.name
            : undefined) ??
          (parent?.key?.name as string | undefined) ??
          'function'
        for (const param of params) {
          const pattern = (() => {
            if (param.type === 'AssignmentPattern') return param.left
            if (param.type === 'RestElement') return param.argument
            return param
          })()
          if (
            (pattern?.type === 'ObjectPattern' ||
              pattern?.type === 'ArrayPattern') &&
            !test
          )
            add({
              metric: 'destructured-parameters',
              node: param,
              text: `${name} destructures a parameter`,
            })
          if (pattern?.name === 'options' && pattern.optional)
            add({
              metric: 'optional-options',
              node: param,
              text: `${name} takes \`options?\` instead of \`options = {}\``,
            })
          const annotation = pattern?.typeAnnotation?.typeAnnotation
          const typeName: string =
            annotation?.typeName?.right?.name ??
            annotation?.typeName?.name ??
            ''
          if (
            /^(args|opts|params)$/.test(pattern?.name ?? '') &&
            (annotation?.type === 'TSTypeLiteral' ||
              /(Args|Options|Parameters|Params)$/.test(typeName))
          )
            add({
              metric: 'option-names',
              node: param,
              text: `${name} names its options \`${pattern.name}\``,
            })
        }
      }

      if (
        node.type === 'VariableDeclarator' &&
        node.id?.typeAnnotation?.typeAnnotation?.type === 'TSTypeLiteral'
      )
        add({
          metric: 'inline-object-types',
          node,
          text: `${node.id.name} has an inline object type`,
        })

      if (
        (node.type === 'BlockStatement' || node.type === 'Program') &&
        Array.isArray(node.body)
      ) {
        node.body.forEach((statement: Node, index: number) => {
          const next: Node | undefined = node.body[index + 1]
          const declarator =
            statement.type === 'VariableDeclaration' &&
            statement.declarations.length === 1
              ? statement.declarations[0]
              : undefined
          const simple =
            declarator?.init && !source.text(declarator.init).includes('\n')
          if (
            simple &&
            next?.type === 'ReturnStatement' &&
            next.argument?.type === 'Identifier' &&
            next.argument.name === declarator.id?.name
          )
            add({
              metric: 'immediate-returns',
              node: statement,
              text: `${declarator.id.name} is returned on the next line`,
            })
          if (
            statement.type === 'VariableDeclaration' &&
            statement.kind === 'let' &&
            declarator &&
            !declarator.init &&
            next?.type === 'TryStatement'
          )
            add({
              metric: 'let-across-try',
              node: statement,
              text: `let ${declarator.id?.name} is assigned across try and catch`,
            })
        })
      }

      if (node.type === 'ThisExpression' && !insideClass(parents))
        add({
          metric: 'this-outside-classes',
          node,
          text: '`this` outside a class',
        })
    })

    for (const type of types) {
      const match = /^(\w+?)(Options|Parameters|ReturnType|ErrorType)$/.exec(
        type.id.name,
      )
      const name = match && match[1]![0]!.toLowerCase() + match[1]!.slice(1)
      if (name && exported.has(name))
        add({
          metric: 'function-namespaces',
          node: type,
          text: `${type.id.name} instead of ${name}.${match![2]}`,
        })
    }

    for (const comment of source.comments)
      if (
        /^[\s*]*([-=#*~_]){3,}/.test(comment.value) &&
        !/cut/.test(comment.value) &&
        !/^\*\s*\n/.test(comment.value)
      )
        (problems['section-dividers'] ??= []).push(
          `line ${source.lineAt(comment.start)}: a section divider`,
        )

    for (const metric of [
      'classes',
      'error-names',
      'uppercase-constants',
      'optional-options',
      'option-names',
      'destructured-parameters',
      'inline-object-types',
      'function-namespaces',
      'immediate-returns',
      'let-across-try',
      'section-dividers',
      'this-outside-classes',
      'module-state',
    ])
      check({ item: file, metric, problems: problems[metric] ?? [] })
  }

  lint(['positional-parameters', 'nested-ternaries', 'math-random'])
  if (production().length > 0) lint(['console-calls'])
  else lines.push('console-calls skip note "no source files under src/"')
}

/**
 * Writes whether each module starts with a `@module` comment, and whether exports and their
 * properties have TSDoc.
 */
function docs(): void {
  for (const [file, source] of sources) {
    const first = source.program.body[0]
    const header = source.comments.find(
      (comment) =>
        comment.type === 'Block' &&
        comment.value.startsWith('*') &&
        (!first || comment.end <= first.start),
    )
    check({
      item: file,
      metric: 'module-docs',
      problems: header?.value.includes('@module')
        ? []
        : ['no TSDoc `@module` comment before the first statement'],
    })

    const exports: string[] = []
    const properties: string[] = []
    let previous: string | undefined
    const visit = (statements: Node[]) => {
      for (const statement of statements) {
        const declaration =
          statement.type === 'ExportNamedDeclaration' ||
          statement.type === 'ExportDefaultDeclaration'
            ? statement.declaration
            : undefined
        if (!declaration) continue
        const name: string =
          declaration.id?.name ??
          declaration.declarations?.[0]?.id?.name ??
          declaration.id?.value ??
          'default'
        const overload = name === previous && /Function/.test(declaration.type)
        previous = name
        if (!overload && !source.documented(statement))
          exports.push(`line ${source.line(statement)}: ${name}`)

        const members: Node[] = (() => {
          if (declaration.type === 'TSInterfaceDeclaration')
            return declaration.body.body
          if (declaration.typeAnnotation?.type === 'TSTypeLiteral')
            return declaration.typeAnnotation.members
          return []
        })()
        for (const member of members)
          if (member.type.endsWith('Signature') && !source.documented(member))
            properties.push(
              `line ${source.line(member)}: ${name}.${member.key?.name ?? member.key?.value ?? '…'}`,
            )
        if (
          declaration.type === 'TSModuleDeclaration' &&
          declaration.body?.body
        )
          visit(declaration.body.body)
      }
    }
    visit(source.program.body)
    check({ item: file, metric: 'export-docs', problems: exports })
    check({ item: file, metric: 'property-docs', problems: properties })
  }
}

/** Writes mocks, inline snapshots, snapshot files, sleeps, and split test files. */
function testing(): void {
  if (tests.length === 0) {
    for (const metric of [
      'mocks',
      'sleeps',
      'split-test-files',
      'snapshot-files',
      'inline-snapshots',
    ])
      lines.push(`${metric} skip note "no test files"`)
    return
  }

  lint(['mocks', 'sleeps'])

  // No linter rule counts inline snapshots or names test files, so the syntax tree answers.
  for (const file of tests) {
    const source = sources.get(file)!
    let expects = 0
    let inline = 0
    walk(source, (node) => {
      const callee = node.type === 'CallExpression' ? node.callee : undefined
      if (callee?.type === 'Identifier' && callee.name === 'expect') expects++
      if (
        /^(toMatchInlineSnapshot|toThrowErrorMatchingInlineSnapshot)$/.test(
          callee?.property?.name ?? '',
        )
      )
        inline++
    })
    check({
      item: file,
      metric: 'split-test-files',
      problems:
        /^[^.]+\.(?!browser\.)[^.]+\.(test|spec|test-d)\.[cm]?tsx?$/.test(
          path.basename(file),
        )
          ? [`${path.basename(file)} splits a module's tests by context`]
          : [],
    })
    check({
      item: file,
      metric: 'inline-snapshots',
      problems:
        inline < expects
          ? [
              `${expects - inline} of ${expects} assertions are not inline snapshots`,
            ]
          : [],
    })
  }

  const snapshots = list(['*.snap']).filter(
    (file) => !ignored.some((pattern) => pattern.test(file)),
  )
  check({
    metric: 'snapshot-files',
    problems: snapshots.map((file) => `${file} is a snapshot file`),
  })
}

/**
 * Writes the project's own lint and format verdicts, then Prettier, catalog versions, script
 * runners, and the key order of each package.json.
 */
function tooling(): void {
  // oxlint and oxfmt read the project's own configuration here, unlike the rule metrics above.
  const own = diagnostics([])
  for (const file of files)
    check({
      item: file,
      metric: 'lint',
      problems: own
        .filter((diagnostic) => diagnostic.file === file)
        .map(describe),
    })

  const formatter = bin('oxfmt')
  if (formatter) {
    const unformatted = new Set(
      Bun.spawnSync([formatter, '--list-different', ...files])
        .stdout.toString()
        .split('\n')
        .map((line) => path.normalize(line.trim())),
    )
    for (const file of files)
      check({
        item: file,
        metric: 'formatted',
        problems: unformatted.has(file) ? ['oxfmt would change this file'] : [],
      })
  } else lines.push('formatted skip note "no oxfmt package in node_modules"')

  const packages = manifests()
  const prettier = [
    ...list(['.prettierrc', '.prettierrc.*', 'prettier.config.*']).map(
      (file) => `${file} configures Prettier`,
    ),
    ...packages
      .filter((manifest) =>
        dependencies(manifest.json).some((entry) => entry[0] === 'prettier'),
      )
      .map((manifest) => `${manifest.file} depends on prettier`),
  ]
  check({ metric: 'prettier', problems: prettier })

  if (packages.length === 0) {
    for (const metric of ['catalog-versions', 'script-runners', 'sorted-keys'])
      lines.push(`${metric} skip note "no package.json"`)
    return
  }
  const workspace = fs.existsSync('pnpm-workspace.yaml')
  for (const manifest of packages) {
    if (workspace)
      check({
        item: manifest.file,
        metric: 'catalog-versions',
        problems: dependencies(manifest.json)
          .filter(
            (entry) =>
              entry[2] !== 'peerDependencies' &&
              !/^(catalog|workspace):/.test(entry[1]),
          )
          .map(
            (entry) => `${entry[0]} is "${entry[1]}", not a catalog reference`,
          ),
      })

    check({
      item: manifest.file,
      metric: 'script-runners',
      problems: Object.entries<string>(manifest.json.scripts ?? {})
        .filter((entry) => /\b(tsx|ts-node)\b/.test(entry[1]))
        .map(
          (entry) =>
            `script ${entry[0]} runs ${/\btsx\b/.test(entry[1]) ? 'tsx' : 'ts-node'}`,
        ),
    })

    check({
      item: manifest.file,
      metric: 'sorted-keys',
      problems: [
        'dependencies',
        'devDependencies',
        'optionalDependencies',
        'peerDependencies',
        'scripts',
      ]
        .filter((key) => {
          const keys = Object.keys(manifest.json[key] ?? {})
          return (
            keys.join() !==
            [...keys]
              .sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()))
              .join()
          )
        })
        .map((key) => `${key} are not in alphabetical order`),
    })
  }
  if (!workspace)
    lines.push('catalog-versions skip note "no pnpm-workspace.yaml"')
}

/** One oxlint diagnostic: its rule, such as `eslint(no-console)`, and where it points. */
type Diagnostic = { file: string; line: number; message: string; rule: string }

/** Writes one check per file for each lint metric, from one oxlint run with the pack's rules. */
function lint(metrics: readonly string[]): void {
  cache.configured ??= diagnostics(['-c', config()])
  const configured = cache.configured
  for (const metric of metrics) {
    const rule = rules[metric]!
    for (const file of rule.files())
      check({
        item: file,
        metric,
        problems: configured
          .filter(
            (diagnostic) => diagnostic.file === file && rule.match(diagnostic),
          )
          .map(describe),
      })
  }
}

/** Runs oxlint on every file, in batches that fit a command line, and returns its diagnostics. */
function diagnostics(flags: readonly string[]): readonly Diagnostic[] {
  const linter = bin('oxlint')
  if (!linter) {
    process.stderr.write(
      'Found no oxlint package in node_modules. Install it with `bun add -d oxlint`\n',
    )
    process.exit(1)
  }
  const found: Diagnostic[] = []
  for (let start = 0; start < files.length; start += 500) {
    const output = Bun.spawnSync([
      linter,
      ...flags,
      '--format',
      'json',
      ...files.slice(start, start + 500),
    ]).stdout.toString()
    const report = JSON.parse(output.slice(output.indexOf('{'))) as {
      diagnostics: {
        code?: string
        filename: string
        labels: { span: { line: number } }[]
        message: string
      }[]
    }
    for (const diagnostic of report.diagnostics)
      found.push({
        file: path.normalize(diagnostic.filename),
        line: diagnostic.labels[0]?.span.line ?? 1,
        message: diagnostic.message,
        rule: diagnostic.code ?? '',
      })
  }
  return found
}

/** Writes the pack's oxlint configuration to a temporary file, and returns its path. */
function config(): string {
  const file = path.join(
    fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-oxlint-')),
    'oxlintrc.json',
  )
  fs.writeFileSync(file, JSON.stringify(oxlint))
  return file
}

function describe(diagnostic: Diagnostic): string {
  return `line ${diagnostic.line}: ${diagnostic.message}${diagnostic.rule ? ` (${diagnostic.rule})` : ''}`
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

/** Returns the files under `src/` that are not tests, where `console` is banned. */
function production(): readonly string[] {
  return files.filter(
    (file) => /(^|\/)src\//.test(file) && !tests.includes(file),
  )
}

/** One problem that `style` records: the metric, the node at fault, and why. */
type Problem = { metric: string; node: Node; text: string }

/** A node of oxc-parser's ESTree-compatible syntax tree. */
// oxlint-disable-next-line typescript/no-explicit-any -- The script reads many node shapes by name.
type Node = any

/**
 * One parsed file: its syntax tree and comments, with helpers that read lines and documentation.
 */
type Source = ReturnType<typeof read>

function read(file: string) {
  const text = fs.readFileSync(file, 'utf8')
  const result = parseSync(file, text)
  const starts = [0]
  for (let index = 0; index < text.length; index++)
    if (text[index] === '\n') starts.push(index + 1)
  const lineAt = (offset: number) => {
    let low = 0
    let high = starts.length - 1
    while (low < high) {
      const middle = (low + high + 1) >> 1
      if (starts[middle]! <= offset) low = middle
      else high = middle - 1
    }
    return low + 1
  }
  const comments = result.comments as {
    end: number
    start: number
    type: 'Block' | 'Line'
    value: string
  }[]

  return {
    comments,
    /** Returns whether a TSDoc comment ends right before the node, with only whitespace between. */
    documented: (node: Node) =>
      comments.some(
        (comment) =>
          comment.type === 'Block' &&
          comment.value.startsWith('*') &&
          text.slice(comment.end, node.start).trim() === '',
      ),
    line: (node: Node) => lineAt(node.start),
    lineAt,
    program: result.program as Node,
    text: (node: Node) => text.slice(node.start, node.end),
  }
}

/** Calls `visit` for every node of the tree, with the node's ancestors from the root down. */
function walk(
  source: Pick<Source, 'program'>,
  visit: (node: Node, parents: Node[]) => void,
): void {
  const parents: Node[] = []
  const step = (node: Node) => {
    visit(node, parents)
    parents.push(node)
    for (const key in node) {
      if (key === 'start' || key === 'end') continue
      const value = node[key]
      if (Array.isArray(value)) {
        for (const child of value)
          if (child && typeof child.type === 'string') step(child)
      } else if (value && typeof value.type === 'string') step(value)
    }
    parents.pop()
  }
  step(source.program)
}

/**
 * Returns whether `this` at the end of `parents` belongs to a class. Arrow functions keep the outer
 * `this`.
 */
function insideClass(parents: Node[]): boolean {
  for (let index = parents.length - 1; index >= 0; index--) {
    const node = parents[index]
    if (node.type === 'ClassBody' || node.type === 'PropertyDefinition')
      return true
    if (
      node.type === 'FunctionDeclaration' ||
      node.type === 'FunctionExpression'
    )
      return parents[index - 1]?.type === 'MethodDefinition'
  }
  return false
}

/** Reads `compilerOptions` from a tsconfig file and the relative files it extends. */
function tsconfig(file: string): Record<string, unknown> | undefined {
  if (!fs.existsSync(file)) return undefined
  const text = fs
    .readFileSync(file, 'utf8')
    .replace(/("(?:\\.|[^"\\])*")|\/\/[^\n]*|\/\*[\s\S]*?\*\//g, '$1')
    .replace(/,(\s*[}\]])/g, '$1')
  const json = JSON.parse(text)
  const bases = (
    Array.isArray(json.extends) ? json.extends : [json.extends]
  ).filter(Boolean) as string[]
  const inherited = bases.map((base) => {
    const resolved = base.startsWith('.')
      ? path.join(path.dirname(file), base)
      : path.join('node_modules', base)
    return (
      tsconfig(resolved.endsWith('.json') ? resolved : `${resolved}.json`) ?? {}
    )
  })
  return Object.assign({}, ...inherited, json.compilerOptions)
}

/** Returns each package.json that Git tracks, outside installed packages. */
function manifests(): { file: string; json: Record<string, Node> }[] {
  return list(['package.json', '*/package.json'])
    .filter((file) => !ignored.some((pattern) => pattern.test(file)))
    .map((file) => ({ file, json: JSON.parse(fs.readFileSync(file, 'utf8')) }))
}

/** Returns each dependency of a package.json as its name, version, and field. */
function dependencies(json: Record<string, Node>): [string, string, string][] {
  return [
    'dependencies',
    'devDependencies',
    'optionalDependencies',
    'peerDependencies',
  ].flatMap((field) =>
    Object.entries<string>(json[field] ?? {}).map(
      (entry): [string, string, string] => [entry[0], entry[1], field],
    ),
  )
}

/** Returns the tracked and untracked files that match `patterns` and that Git does not ignore. */
function list(patterns: string[]): string[] {
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
        ...new Bun.Glob(`**/${pattern}`).scanSync({ dot: true }),
      ])
  return [...new Set(found)]
    .filter((file) => file && fs.existsSync(file))
    .sort()
}

/**
 * Writes a check for one item: it passes without problems, and fails with the first as its note.
 */
function check(options: check.Options): void {
  const { item, metric, problems } = options
  const subject = item === undefined ? metric : `${metric} ${quote(item)}`
  const more = problems.length > 1 ? ` (and ${problems.length - 1} more)` : ''
  lines.push(
    problems.length === 0
      ? `${subject} pass`
      : `${subject} fail${note(`${problems[0]}${more}`)}`,
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
    item?: string | undefined
    metric: string
    problems: readonly string[]
  }
}
