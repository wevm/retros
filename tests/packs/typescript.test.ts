/**
 * Runs the TypeScript pack in an independent checkout with its own compiler and formatter.
 * @module
 */
import * as assert from 'node:assert/strict'
import * as child from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import * as test from 'node:test'
import * as url from 'node:url'

const repo = url.fileURLToPath(new URL('../../', import.meta.url))

test.test(
  'TypeScript pack checks participating files and preserves tool failures',
  { timeout: 60_000 },
  () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-ts-test-'))
    const pack = path.join(dir, 'pack')
    const project = path.join(dir, 'project')
    const metrics = path.join(dir, 'metrics')

    function write(file: string, contents: string) {
      const target = path.join(project, file)
      fs.mkdirSync(path.dirname(target), { recursive: true })
      fs.writeFileSync(target, contents)
    }

    function run() {
      fs.writeFileSync(metrics, '')
      return child.spawnSync(
        'bun',
        [path.join(pack, 'metrics/typescript.ts')],
        {
          cwd: project,
          encoding: 'utf8',
          env: { ...process.env, UPHILL_METRICS: metrics },
          timeout: 30_000,
        },
      )
    }

    try {
      fs.mkdirSync(project)
      fs.mkdirSync(pack)
      for (const name of ['package.json', 'bun.lock']) {
        fs.copyFileSync(
          path.join(repo, 'packs/typescript', name),
          path.join(pack, name),
        )
        fs.copyFileSync(
          path.join(repo, 'examples/typescript-library', name),
          path.join(project, name),
        )
      }
      fs.cpSync(
        path.join(repo, 'packs/typescript/metrics'),
        path.join(pack, 'metrics'),
        { recursive: true },
      )
      for (const cwd of [pack, project])
        child.execFileSync(
          'bun',
          ['install', '--frozen-lockfile', '--ignore-scripts'],
          { cwd },
        )

      write('.gitignore', 'node_modules/\n')
      write('.eslintignore', 'ignored.ts\n')
      write('.prettierignore', 'ignored.ts\n')
      write('tsconfig.json', '{"files":[],"references":[{"path":"./lib"}]}')
      write(
        'node_modules/base/package.json',
        '{"name":"base","main":"tsconfig.json"}',
      )
      write(
        'node_modules/base/tsconfig.json',
        '{"compilerOptions":{"strict":true,"verbatimModuleSyntax":true}}',
      )
      write(
        'lib/tsconfig.json',
        '{"extends":"base","compilerOptions":{"module":"nodenext","types":[],"composite":true}}',
      )
      write('lib/Bad.ts', 'export const x: string = 1;\n')
      write('lib/Good.ts', 'export const x = 1;\n')
      write('excluded.ts', 'export const x: string = 1;\n')
      write('ignored.ts', 'export type Any = any;\n')
      write('public.d.ts', 'export type Any = any;\n')
      write('broken.ts', 'enum A { One }\nexport const = ;\n')
      write(
        'lib/Store.ts',
        '/** @module Store. */\nexport type Entry = { value: string };\n/** Later only. */\nexport const value = 42;\n',
      )
      write('reexport.ts', "export { Entry } from './lib/Store.js';\n")
      write(
        'correct.ts',
        "export { type Entry, value } from './lib/Store.js';\n",
      )
      write(
        'alias.ts',
        '/** @license MIT */\n/** @module Types. */\ntype Shape = { property: number };\nexport type { Shape as Public };\n',
      )
      write(
        'consumer.tsx',
        "import { value } from './lib/Store.js';\nexport { value };\n",
      )
      write('Button.tsx', 'export function Button() { return null }\n')
      write(
        'component.tsx',
        "import { Button } from './Button.js';\nexport { Button };\n",
      )
      write(
        'errors.ts',
        "export class OneError extends Error { ['name'] = 'Type.OneError' }\n",
      )
      write(
        'static.ts',
        "export class TwoError extends Error { static name = 'Type.TwoError' }\n",
      )
      write(
        'try.ts',
        'export function read() { let v; try { return 1 } catch { } v = 2; return v }\n',
      )
      write('mutate.ts', 'export function append(a: number[]) { a.push(1) }\n')
      write(
        'view.ts',
        'export function view(a: number[]) { return a.length }\n',
      )
      write(
        'scope_test.ts',
        "import {expect as check, mock} from 'bun:test';\ncheck(mock(() => 1)()).toBe(1); setTimeout(() => {}, 1);\n",
      )
      write(
        'control.test.ts',
        "import * as t from 'bun:test';\nt.expect(1).toMatchInlineSnapshot('1');\nfunction locally(expect: (n: number) => number) { return expect(2) }\n",
      )
      write('stored.snap', '1\n')
      write(
        'pnpm-workspace.yaml',
        'packages: ["packages/*", "!packages/excluded"]\n',
      )
      write(
        'packages/lib/package.json',
        '{"name":"lib","bin":"cli.ts","prettier":{},"dependencies":{"a":"1.0.0"},"scripts":{"ui":"bun ui.tsx"}}',
      )
      write(
        'packages/excluded/package.json',
        '{"name":"private","main":"index.js","private":true,"dependencies":{"a":"1.0.0"},"scripts":{"test":"tsx test.ts"}}',
      )
      child.execFileSync('git', ['init', '-q'], { cwd: project })
      child.execFileSync('git', ['add', '.'], { cwd: project })
      write('untracked.ts', 'export type Any = any;\n')

      const result = run()
      assert.equal(result.status, 0, result.stderr)
      const report = fs.readFileSync(metrics, 'utf8')
      for (const line of [
        'type-errors "lib/Bad.ts" fail note "line 1: TS2322:',
        'type-errors "lib/Good.ts" pass',
        'type-errors "excluded.ts" skip',
        'compiler-options "lib/tsconfig.json: strict" pass',
        'explicit-any "public.d.ts" fail',
        'explicit-any "ignored.ts" fail',
        'lint "ignored.ts" skip',
        'formatted "ignored.ts" skip',
        'enums "broken.ts" skip note "oxc-parser: line 2:',
        'formatted "broken.ts" skip',
        'formatted "lib/Good.ts" pass',
        'formatted "scope_test.ts" fail',
        'type-imports "reexport.ts" fail',
        'type-imports "correct.ts" pass',
        'namespace-imports "consumer.tsx" fail',
        'namespace-imports "component.tsx" pass',
        'export-docs "lib/Store.ts" fail',
        'property-docs "lib/Store.ts" fail',
        'module-docs "alias.ts" pass',
        'export-docs "alias.ts" fail',
        'property-docs "alias.ts" fail',
        'error-names "errors.ts" pass',
        'error-names "static.ts" fail',
        'let-across-try "try.ts" pass',
        'mutable-arrays "mutate.ts" pass',
        'mutable-arrays "view.ts" fail',
        'mocks "scope_test.ts" fail',
        'sleeps "scope_test.ts" fail',
        'inline-snapshots "scope_test.ts" fail',
        'inline-snapshots "control.test.ts" pass',
        'snapshot-files fail',
        'side-effects "packages/lib/package.json" fail',
        'catalog-versions "packages/lib/package.json" fail',
        'script-runners "packages/lib/package.json" pass',
        'script-runners "packages/excluded/package.json" fail',
      ])
        assert.ok(
          report.split('\n').some((actual) => actual.startsWith(line)),
          line,
        )
      assert.doesNotMatch(
        report,
        /"untracked\.ts"|(?:side-effects|catalog-versions) "packages\/excluded\/package\.json"/,
      )
      assert.ok(!fs.existsSync(path.join(project, 'lib/tsconfig.tsbuildinfo')))

      write(
        'tsconfig.json',
        '{"files":["missing.ts"],"compilerOptions":{"types":[]}}',
      )
      const missing = run()
      assert.equal(missing.status, 0, missing.stderr)
      const incomplete = fs.readFileSync(metrics, 'utf8')
      assert.match(
        incomplete,
        /^type-errors "tsconfig\.json" fail note "TS6053: File 'missing\.ts' not found\."$/m,
      )
      assert.match(incomplete, /^type-errors "lib\/Good\.ts" skip /m)

      write('.oxfmtrc.json', '{"semi":"invalid"}')
      const fmt = run()
      assert.equal(fmt.status, 0, fmt.stderr)
      const noFormat = fs.readFileSync(metrics, 'utf8')
      assert.match(
        noFormat,
        /^formatted skip note .*Failed to parse configuration/m,
      )
      assert.match(noFormat, /^explicit-any "ignored.ts" fail/m)
      assert.doesNotMatch(noFormat, /^formatted .* pass$/m)

      fs.unlinkSync(path.join(project, '.oxfmtrc.json'))
      write('.oxlintrc.json', '{"extends":["./missing-rules.json"]}')
      const lint = run()
      assert.equal(lint.status, 0, lint.stderr)
      const noLint = fs.readFileSync(metrics, 'utf8')
      assert.match(noLint, /^lint skip note .*missing-rules\.json/m)
      assert.match(noLint, /^formatted "lib\/Good\.ts" pass$/m)

      write('package.json', '{"devDependencies":{"eslint":"*","prettier":"*"}}')
      const absent = run()
      assert.equal(absent.status, 0, absent.stderr)
      const notInstalled = fs.readFileSync(metrics, 'utf8')
      assert.match(
        notInstalled,
        /^lint skip note "eslint is in package.json but not in node_modules"$/m,
      )
      assert.match(
        notInstalled,
        /^formatted skip note "prettier is in package.json but not in node_modules"$/m,
      )
      assert.match(notInstalled, /^explicit-any "ignored.ts" fail/m)
    } finally {
      fs.rmSync(dir, { force: true, recursive: true })
    }
  },
)
