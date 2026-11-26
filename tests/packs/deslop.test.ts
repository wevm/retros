/** @module Exercises architecture scores, attribution and incomplete graph coverage at the Uphill CLI. */
import * as assert from 'node:assert/strict'
import * as child from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const repo = fileURLToPath(new URL('../../', import.meta.url))
type Report = {
  categories: {
    score: number | null
    metrics: {
      name: string
      score: number | null
      items: { name: string; value: boolean | null; note: string }[]
    }[]
  }[]
}

function project(exercise: (cwd: string) => void) {
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'deslop-project-'))
  try {
    child.execFileSync('git', ['init', '-q'], { cwd })
    write(cwd, {
      'uphill.yaml': `packs: [${JSON.stringify(path.relative(cwd, path.join(repo, 'packs/deslop')))}]`,
    })
    exercise(cwd)
  } finally {
    fs.rmSync(cwd, { recursive: true, force: true })
  }
}

function write(cwd: string, files: Record<string, string>) {
  for (const [file, text] of Object.entries(files)) {
    const full = path.join(cwd, file)
    fs.mkdirSync(path.dirname(full), { recursive: true })
    fs.writeFileSync(full, `${text}\n`)
  }
}

function measure(cwd: string, status: number) {
  const result = child.spawnSync(
    path.join(repo, 'target/debug/uphill'),
    ['--compact', '--require-targets'],
    {
      cwd,
      encoding: 'utf8',
      env: { ...process.env, CARGO_TARGET_DIR: path.join(cwd, 'target') },
      timeout: 90_000,
    },
  )
  if (result.error) throw result.error
  assert.equal(result.status, status, result.stdout + result.stderr)
  const report = JSON.parse(
    fs.readFileSync(path.join(cwd, '.uphill/report.json'), 'utf8'),
  ) as Report
  const category = report.categories[0]!
  const item = (metric: string, file: string) => {
    const entry = category.metrics
      .find((m) => m.name === metric)
      ?.items.find((i) => i.name === file)
    assert.ok(entry, `missing ${metric} ${file}: ${result.stdout}`)
    return entry
  }
  return { category, item }
}

test('source graphs include real edges but exclude inert source text', () =>
  project((cwd) => {
    write(cwd, {
      'go.mod': 'module example.org/sample\n\ngo 1.24',
      'a/lib.go': 'package a\nconst text = `import "example.org/sample/b"`',
      'b/lib.go': 'package b\nimport (_ "example.org/sample/a")',
      'a.h': '/*\n#include "b.h"\n*/',
      'b.h': '#include "a.h"',
      'a.ts': `import './b.js'; export {}`,
      'b.ts': `export const text = "import './a.js'"`,
      // A runtime built-in is outside the project, so the program stays complete without its types.
      'c.ts': `import * as fs from 'node:fs'; export const read = fs.readFileSync`,
      'a.zig': 'const b = @import( // local module\n "b.zig");',
      'b.zig': '// @import("a.zig")',
      'A.swift': 'func first() -> Int { second() }',
      'B.swift': 'func second() -> Int { 1 }',
    })
    const items = [
      'a/',
      'b/',
      'a.h',
      'b.h',
      'a.ts',
      'b.ts',
      'a.zig',
      'b.zig',
      'A.swift',
      'B.swift',
    ]
    const clean = measure(cwd, 0)
    for (const file of [...items, 'c.ts'])
      assert.equal(clean.item('module-cycle', file).value, true, file)

    write(cwd, {
      'a/lib.go': 'package a\nimport (\n_ `example.org/sample/b`\n)',
      'a.h': '#include "b.h"',
      'b.ts': `import './a.js'`,
      'b.zig': 'const a = @import("a.zig");',
      'B.swift': 'func second() -> Int { first() }',
    })
    const cycles = measure(cwd, 1)
    for (const file of items)
      assert.equal(cycles.item('module-cycle', file).value, false, file)

    write(cwd, {
      'a.h': '#include HEADER',
      'b.ts': `void import('./' + name)`,
      'b.zig': 'const a = @import("app");',
      'B.swift': 'func second() -> Int { object.first() }',
    })
    const unknown = measure(cwd, 1)
    for (const file of ['a.h', 'b.ts', 'b.zig', 'B.swift'])
      assert.equal(unknown.item('module-cycle', file).value, null, file)
  }))

test('TypeScript binds field receivers and aliased calls to their owners', () =>
  project((cwd) => {
    const functions = Array.from({ length: 8 }, (_, i) => `fn${i}`)
    write(cwd, {
      'Api.ts': functions
        .map((name) => `export function ${name}() { return 1 }`)
        .join('\n'),
      'Queue.ts':
        'export class Queue { entries: number[] = []; private capacity = 4; full() { return this.entries.length >= this.capacity } }',
      'Use.ts': `import { ${functions.map((name) => `${name} as local${name}`).join(', ')} } from './Api.js';\n${functions.map((name) => `local${name}();`).join('\n')}\nconst other = { entries: [] as number[] }; other.entries.push(2)`,
    })
    const aliased = measure(cwd, 1)
    assert.equal(aliased.item('field-leak', 'Queue.ts').value, true)
    assert.equal(aliased.item('shallow-module', 'Api.ts').value, false)

    write(cwd, {
      'Use.ts': `import {Queue as Buffer} from './Queue.js'; const value = new Buffer(); value.entries.push(2)`,
    })
    const leak = measure(cwd, 1)
    assert.equal(leak.item('field-leak', 'Queue.ts').value, false)
    assert.match(leak.item('field-leak', 'Queue.ts').note, /Use\.ts:1/)
    assert.equal(leak.item('shallow-module', 'Api.ts').value, true)

    write(cwd, {
      'Use.ts': `declare const unknown: any; unknown.entries.push(2); export {}`,
    })
    assert.equal(measure(cwd, 0).item('field-leak', 'Queue.ts').value, null)
  }))

test('only verified Cargo production inputs count toward the score', () =>
  project((cwd) => {
    write(cwd, {
      'Cargo.toml':
        '[package]\nname="sample"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[features]\nextra=[]',
      'src/lib.rs':
        'pub mod a; pub mod b; #[cfg(feature="extra")] pub mod extra;',
      'src/a.rs': 'use crate::b::MAX; pub fn next() -> u32 { MAX + 1 }',
      'src/b.rs':
        'use crate::a::next; pub const MAX: u32 = 3; pub fn limit() -> u32 { next() }',
      'src/extra.rs': 'pub fn optional() {}',
    })
    const before = measure(cwd, 1)
    assert.equal(before.category.score, 83)
    for (const file of ['src/a.rs', 'src/b.rs'])
      assert.equal(before.item('module-cycle', file).value, false)
    assert.equal(before.item('module-cycle', 'src/extra.rs').value, null)

    write(cwd, {
      'src/orphan.rs': 'pub struct Orphan;',
      'outside/unused.rs': 'pub struct Unused;',
    })
    const after = measure(cwd, 1)
    assert.equal(after.category.score, before.category.score)
    for (const metric of after.category.metrics) {
      for (const file of ['src/orphan.rs', 'outside/unused.rs'])
        assert.equal(after.item(metric.name, file).value, null)
    }

    write(cwd, {
      'src/lib.rs': 'pub mod a; pub mod b;',
      'src/a.rs':
        'pub struct Queue {\n pub entries: Vec<u32>,\n capacity: usize,\n}\nimpl Queue { pub fn full(&self) -> bool { self.entries.len() >= self.capacity } }',
      'src/b.rs':
        'use crate::a::Queue;\npub fn append(queue: &mut Queue) { queue.entries.push(1); }',
    })
    const leak = measure(cwd, 1)
    assert.equal(leak.item('field-leak', 'src/a.rs').value, false)
    assert.match(leak.item('field-leak', 'src/a.rs').note, /src\/b\.rs:2/)
    assert.equal(leak.item('field-leak', 'src/b.rs').value, true)
    assert.equal(leak.item('module-cycle', 'src/b.rs').value, true)

    write(cwd, { 'src/b.rs': 'pub fn broken(' })
    assert.equal(measure(cwd, 1).category.score, null)
  }))
