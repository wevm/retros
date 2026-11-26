/** @module Exercises the TigerStyle pack as a process against a Git index, not isolated scanner helpers. */
import assert from 'node:assert/strict'
import * as child from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import { test } from 'node:test'

const entry = path.resolve(
  import.meta.dirname,
  '../../packs/tigerstyle/metrics/tigerstyle.ts',
)

function measure(cwd: string) {
  const metrics = path.join(cwd, 'output')
  fs.writeFileSync(metrics, '')
  child.execFileSync('bun', [entry], {
    cwd,
    env: { ...process.env, UPHILL_METRICS: metrics },
  })
  return fs.readFileSync(metrics, 'utf8').split('\n')
}

test('measures tracked functions, not signature syntax, declarations, test bodies, or literal text', () => {
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'tigerstyle-test-'))
  const write = (name: string, text: string) =>
    fs.writeFileSync(path.join(cwd, name), text)
  const git = (...args: string[]) => child.execFileSync('git', args, { cwd })

  const cases = [
    {
      ext: 'c',
      header: '#include <assert.h>',
      start: 'int (*walk(int n))(int) {',
      call: 'if (n > 0) return walk(n-1);',
      end: 'return 0;',
      assert: 'assert',
      op: '&&',
      semi: ';',
    },
    {
      ext: 'go',
      header: 'package main',
      start: 'func walk(n int) struct{ x int } {',
      call: 'if n > 0 { return walk(n-1) }',
      end: 'return struct{ x int }{n}',
      assert: 'assert',
      op: '&&',
      semi: '',
    },
    {
      ext: 'rs',
      header: '',
      start: 'pub fn walk(n: [i32; 1]) -> i32 {',
      call: 'if n[0] > 0 { return walk([n[0]-1]); }',
      end: 'n[0]',
      assert: 'assert!',
      op: '&&',
      semi: ';',
    },
    {
      ext: 'swift',
      header: '',
      start: 'func walk(_ n: Int, apply: (Int) -> Int = { $0 }) -> Int {',
      call: 'if n > 0 { return walk(n-1) }',
      end: 'return apply(n)',
      assert: 'precondition',
      op: '&&',
      semi: '',
    },
    {
      ext: 'ts',
      header: '',
      start: 'export function walk(n: number): { x: number } {',
      call: 'if (n > 0) return walk(n-1);',
      end: 'return { x: n };',
      assert: 'console.assert',
      op: '&&',
      semi: ';',
    },
    {
      ext: 'zig',
      header: 'const std = @import("std");',
      start: 'pub fn walk(n: i32) error{Bad}!i32 {',
      call: 'if (n > 0) return walk(n-1);',
      end: 'return n;',
      assert: 'std.debug.assert',
      op: 'and',
      semi: ';',
    },
  ]

  try {
    git('init', '-q')
    for (const lang of cases) {
      for (const bad of [false, true]) {
        const condition = lang.ext === 'c' ? '1' : 'true'
        const check = (expr: string) =>
          `    ${lang.assert}(${expr})${lang.semi}`
        const body = [
          lang.start,
          check(bad ? `${condition} ${lang.op} ${condition}` : condition),
          bad ? `    ${lang.call}` : check(condition),
        ]
        while (body.length < (bad ? 71 : 70) - 2)
          body.push(
            `    // Ignored: walk() ${lang.assert}(false ${lang.op} false) }.`,
          )
        body.push('    ' + lang.end, '}')
        write(
          `${bad ? 'bad' : 'good'}.${lang.ext}`,
          [lang.header, '//'.padEnd(bad ? 101 : 100, '.'), ...body].join('\n'),
        )
      }
    }
    write(
      'lexical.rs',
      'fn harmless() { /* outer /* inner */ harmless(); assert!(false && false); usize */ }\n#[cfg(test)]\nmod tests { fn check() { assert!(true && false); } }',
    )
    write(
      'literal.swift',
      'func sample() -> String {\n    return """\n    sample(); assert(true && false)\n    """\n}',
    )
    write('callback.c', 'void notify(void (*notify)(void)) { notify(); }')
    write(
      'probe.cts',
      'const inner = (n: number): string => n ? `${inner(n-1)}` : "done";\nfunction check(and: boolean) { console.assert(and); }',
    )
    write(
      'module.go',
      'package main\ntype counter struct{}\nfunc (c counter) step(n int) int { if n>0 {return c.step(n-1)}; return n }\nfunc assert(ok bool) { if !ok { panic("error") } }',
    )
    write(
      'conditional.zig',
      'const std = @import("std");\npub fn one() i32 { return 1; }\ntest "one" { std.debug.assert(one() == 1 and true); }',
    )
    write(
      'go.mod',
      'module example.org/main\nexclude (\n example.org/obsolete v1.0.0\n)\n',
    )
    write('package.json', '{"optionalDependencies":{"optional":"1"}}')
    write(
      'build.zig.zon',
      '.{ .dependencies = .{ .utility = .{ .path = "../utility" } } }',
    )
    write(
      'Package.swift',
      '// swift-tools-version: 5.9\nimport PackageDescription\nlet package = Package(name: "Probe", dependencies: [.package(path: "../dep")])',
    )
    git('add', '.')
    write('untracked.ts', 'function recur() { return recur() }')

    const lines = measure(cwd)
    function has(metric: string, item: string, value: string) {
      const prefix = `${metric} "${item}" ${value}`
      assert.ok(
        lines.some(
          (line) => line === prefix || line.startsWith(prefix + ' note '),
        ),
        prefix,
      )
    }
    for (const lang of cases) {
      for (const bad of [false, true]) {
        const file = `${bad ? 'bad' : 'good'}.${lang.ext}`
        for (const metric of [
          'function-length',
          'recursion',
          'line-length',
          'compound-assertions',
        ])
          has(metric, file, bad ? 'fail' : 'pass')
        has('assertion-density', file, bad ? '1.00' : '2.00')
        has('function-lines', file + '::walk', bad ? '71' : '70')
      }
    }
    for (const file of ['lexical.rs', 'literal.swift', 'conditional.zig']) {
      has('assertion-density', file, '0.00')
      has('compound-assertions', file, 'pass')
      has('recursion', file, 'pass')
    }
    has('sized-integers', 'lexical.rs', 'pass')
    has('recursion', 'callback.c', 'pass')
    has('recursion', 'module.go', 'fail')
    has('assertion-density', 'module.go', '0.00')
    has('recursion', 'probe.cts', 'fail')
    has('compound-assertions', 'probe.cts', 'pass')
    has('zero-dependencies', 'go.mod', 'pass')
    for (const file of ['build.zig.zon', 'package.json', 'Package.swift'])
      has('zero-dependencies', file, 'fail')
    assert.ok(!lines.some((line) => line.includes('untracked.ts')))

    // The file still fails. The stable function item records the three-line change.
    const bad = fs.readFileSync(path.join(cwd, 'bad.ts'), 'utf8')
    write(
      'bad.ts',
      bad.replace('    return { x: n };', '\n\n\n    return { x: n };'),
    )
    write('broken.ts', 'export function incomplete( {')
    git('add', 'broken.ts')
    const changed = measure(cwd)
    assert.ok(
      changed.some((line) =>
        line.startsWith('function-lines "bad.ts::walk" 74 note '),
      ),
    )
    assert.ok(
      changed.some((line) =>
        line.startsWith('function-length "bad.ts" fail note '),
      ),
    )
    assert.ok(
      changed.some((line) =>
        line.startsWith('source-coverage "broken.ts" fail note '),
      ),
    )
    assert.ok(
      changed.some((line) =>
        line.startsWith('recursion "broken.ts" skip note '),
      ),
    )
  } finally {
    fs.rmSync(cwd, { force: true, recursive: true })
  }
})

test('attributes only length diagnostics to the checked file and measures files outside the Cargo graph', (t) => {
  if (child.spawnSync('cargo', ['clippy', '--version']).status !== 0) {
    t.skip('Clippy is not installed')
    return
  }
  const oxlint = path.resolve(
    import.meta.dirname,
    '../../packs/typescript/node_modules/oxlint',
  )
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'tigerstyle-linters-'))
  try {
    fs.mkdirSync(path.join(cwd, 'src'))
    fs.mkdirSync(path.join(cwd, 'node_modules'))
    fs.symlinkSync(oxlint, path.join(cwd, 'node_modules/oxlint'), 'dir')
    fs.writeFileSync(
      path.join(cwd, 'Cargo.toml'),
      '[workspace]\n[package]\nname="probe"\nversion="0.0.0"\nedition="2024"\n',
    )
    const rust =
      'pub fn run(mut x: i32) -> i32 {\n' +
      '    x += 1;\n'.repeat(73) +
      '    x\n}\n'
    fs.writeFileSync(path.join(cwd, 'src/lib.rs'), rust)
    fs.writeFileSync(path.join(cwd, 'orphan.rs'), rust)
    fs.writeFileSync(
      path.join(cwd, 'unrelated.ts'),
      'export function valid(n: number) { return n === NaN; }\n',
    )
    fs.writeFileSync(
      path.join(cwd, 'length.ts'),
      'export function work(n: number) {\n' +
        '    n += 1;\n'.repeat(73) +
        '    return n;\n}\n',
    )
    child.execFileSync('git', ['init', '-q'], { cwd })
    child.execFileSync(
      'git',
      [
        'add',
        '--',
        'Cargo.toml',
        'src',
        'orphan.rs',
        'length.ts',
        'unrelated.ts',
      ],
      { cwd },
    )

    const measured = measure(cwd)
    assert.match(
      measured.join('\n'),
      /^function-length "src\/lib.rs" fail .*clippy::too_many_lines/m,
    )
    assert.match(
      measured.join('\n'),
      /^function-length "orphan.rs" fail note "line 1: run has 76 lines"$/m,
    )
    assert.match(
      measured.join('\n'),
      /^function-length "length.ts" fail .*eslint\/max-lines-per-function/m,
    )
    assert.ok(measured.includes('function-length "unrelated.ts" pass'))

    fs.appendFileSync(
      path.join(cwd, 'src/lib.rs'),
      'compile_error!("stop before linting");\n',
    )
    const interrupted = measure(cwd)
    assert.ok(
      interrupted.includes(
        'function-length "src/lib.rs" fail note "line 1: run has 76 lines"',
      ),
    )
    assert.ok(
      interrupted.includes(
        'function-length "orphan.rs" fail note "line 1: run has 76 lines"',
      ),
    )
  } finally {
    fs.rmSync(cwd, { force: true, recursive: true })
  }
})

test('the installed assertion-density target gates a file with zero assertions, not one with two', () => {
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'tigerstyle-gate-'))
  const pack = path.resolve(path.dirname(entry), '..')
  const binary = path.resolve(pack, '../../target/debug/uphill')
  try {
    child.execFileSync('git', ['init', '-q'], { cwd })
    fs.mkdirSync(path.join(cwd, '.agents/uphill/packs'), { recursive: true })
    fs.symlinkSync(
      pack,
      path.join(cwd, '.agents/uphill/packs/tigerstyle'),
      'dir',
    )
    fs.writeFileSync(
      path.join(cwd, 'uphill.yaml'),
      'packs: [./.agents/uphill/packs/tigerstyle]\n',
    )
    for (const n of [0, 2]) {
      fs.writeFileSync(
        path.join(cwd, 'identity.ts'),
        'export function identity(n: number) {\n' +
          '    console.assert(n >= 0);\n'.repeat(n) +
          '    return n;\n}\n',
      )
      child.execFileSync('git', ['add', 'identity.ts'], { cwd })
      const run = child.spawnSync(binary, ['--require-targets'], {
        cwd,
        encoding: 'utf8',
        timeout: 30_000,
      })
      assert.ifError(run.error)
      assert.equal(run.status, n ? 0 : 1, run.stderr + run.stdout)
      type Metric = {
        name: string
        target: number
        value: number
        score: number
      }
      const report = JSON.parse(
        fs.readFileSync(path.join(cwd, '.uphill/report.json'), 'utf8'),
      ) as { categories: { metrics: Metric[]; name: string }[] }
      const density = report.categories
        .find((c) => c.name === 'correctness')
        ?.metrics.find((m) => m.name === 'assertion-density')
      assert.equal(density?.target, 2)
      assert.equal(density?.value, n)
      assert.equal(density?.score, n ? 90 : 0)
    }
  } finally {
    fs.rmSync(cwd, { force: true, recursive: true })
  }
})
