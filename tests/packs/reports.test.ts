/** @module Exercises pack scores, skipped items, and gates through the installed CLI contract. */
import * as assert from 'node:assert/strict'
import * as childProcess from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import { test } from 'node:test'

const repo = path.resolve(import.meta.dirname, '../..')

function project(packs: readonly string[], exercise: (cwd: string) => void) {
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-pack-'))
  try {
    childProcess.execFileSync('git', ['init', '-q'], { cwd })
    const local = packs.map((pack) =>
      path.relative(cwd, path.join(repo, 'packs', pack)),
    )
    fs.writeFileSync(
      path.join(cwd, 'uphill.yaml'),
      `packs: ${JSON.stringify(local)}\n`,
    )
    exercise(cwd)
  } finally {
    fs.rmSync(cwd, { recursive: true, force: true })
  }
}

function report(cwd: string) {
  const run = childProcess.spawnSync(
    path.join(repo, 'target/debug/uphill'),
    ['--compact', '--require-targets'],
    { cwd, encoding: 'utf8', timeout: 30_000 },
  )
  assert.ifError(run.error)
  const file = path.join(cwd, '.uphill/report.json')
  assert.ok(fs.existsSync(file), run.stderr)
  const { categories } = JSON.parse(fs.readFileSync(file, 'utf8')) as {
    categories: {
      error?: { details: string }
      metrics?: {
        items: { name: string; value: boolean | null }[]
        name: string
        score: number | null
      }[]
      name: string
      score: number | null
    }[]
  }
  const metrics = categories.flatMap((category) => category.metrics ?? [])
  const item = (metric: string, name: string) =>
    metrics.find((m) => m.name === metric)?.items.find((i) => i.name === name)
      ?.value
  return { categories, item, metrics, status: run.status }
}

test('web scores matching selectors, ignores unrelated lint, and cannot pass unparseable CSS', () => {
  project(['web'], (cwd) => {
    const css = path.join(cwd, 'screen.css')
    fs.writeFileSync(path.join(cwd, '.stylelintignore'), 'ignored.css\n')
    fs.writeFileSync(
      path.join(cwd, 'ignored.css'),
      '.ignored:focus { outline: none; }',
    )
    fs.writeFileSync(
      path.join(cwd, 'button.tsx'),
      'export function Next() { const unused = 1; return <button>Next</button> }',
    )
    const badFocus =
      '.one:hover, .other:focus { color: var(--ink); }\n.two:hover { color: var(--ink); }'
    const motion =
      '.spin, .pulse { animation: spin 2s; @media (prefers-reduced-motion: reduce) { padding: 2px; } }'
    fs.writeFileSync(css, `${badFocus}\n${motion}`)
    const before = report(cwd)
    assert.equal(before.status, 1)
    assert.equal(
      before.metrics.find((m) => m.name === 'focus-styles')?.score,
      0,
    )
    assert.equal(before.item('controls', 'button.tsx'), true)
    assert.equal(before.item('focus-outlines', 'ignored.css'), null)
    assert.equal(
      before.item('reduced-motion', 'screen.css .spin animation'),
      false,
    )

    fs.appendFileSync(
      css,
      '\n.one:focus-visible { color: var(--ink); }\n@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }',
    )
    const partial = report(cwd)
    assert.equal(partial.status, 1)
    assert.equal(
      partial.metrics.find((m) => m.name === 'focus-styles')?.score,
      50,
    )
    assert.equal(
      partial.item('reduced-motion', 'screen.css .spin animation'),
      true,
    )
    assert.equal(
      partial.item('reduced-motion', 'screen.css .pulse animation'),
      false,
    )

    fs.writeFileSync(css, '.one:focus { outline: 2px solid; }')
    assert.equal(report(cwd).status, 0)
    fs.writeFileSync(css, '.one:focus { outline: none;')
    const invalid = report(cwd)
    assert.equal(invalid.status, 1)
    const accessibility = invalid.categories.find(
      (category) => category.name === 'accessibility',
    )
    assert.equal(accessibility?.score, null)
    assert.match(
      accessibility?.error?.details ?? '',
      /screen\.css:.*Unclosed block/,
    )
  })
})

test('git distinguishes working-tree checks from warnings and test paths in commit history', () => {
  project(['git'], (cwd) => {
    const git = (...args: string[]) =>
      childProcess
        .execFileSync(
          'git',
          [
            '-c',
            'user.name=Fixture',
            '-c',
            'user.email=fixture@example.invalid',
            '-c',
            'commit.gpgsign=false',
            ...args,
          ],
          { cwd, encoding: 'utf8' },
        )
        .trim()
    fs.mkdirSync(path.join(cwd, 'tests'))
    fs.mkdirSync(path.join(cwd, '.github/workflows'), { recursive: true })
    fs.writeFileSync(
      path.join(cwd, '.github/workflows/example.yml'),
      "'name': Build\n'on': push\njobs: {}\n",
    )
    fs.writeFileSync(path.join(cwd, 'a.txt'), 'clean\n')
    fs.writeFileSync(path.join(cwd, 'b.txt'), 'old error \n')
    git('add', '.')
    git('commit', '-qm', 'feat: speed up parsing')
    const base = git('rev-parse', '--short', 'HEAD')
    fs.writeFileSync(path.join(cwd, 'tests/README.md'), 'Test notes.\n')
    git('add', '.')
    git('commit', '-qm', 'fix: wrote files\nMissing blank line.')
    const fix = git('rev-parse', '--short', 'HEAD')
    fs.writeFileSync(path.join(cwd, 'a.txt'), 'new error \n')
    fs.writeFileSync(path.join(cwd, 'b.txt'), 'clean\n')

    const result = report(cwd)
    assert.equal(result.item('whitespace', 'a.txt'), false)
    assert.equal(result.item('whitespace', 'b.txt'), true)
    assert.equal(
      result.item('workflow-keys', '.github/workflows/example.yml'),
      true,
    )
    assert.equal(result.item('imperative-subjects', base), true)
    assert.equal(result.item('imperative-subjects', fix), false)
    assert.equal(result.item('message-body', fix), false)
    assert.equal(result.item('fix-tests', fix), false)
  })
})

test('Rust and TypeScript merge empty test populations without scoring them', () => {
  project(['rust', 'typescript'], (cwd) => {
    fs.mkdirSync(path.join(cwd, 'src'))
    fs.writeFileSync(
      path.join(cwd, 'Cargo.toml'),
      '[package]\nname="mixed"\nversion="0.0.0"\nedition="2024"\n[workspace]\n',
    )
    fs.writeFileSync(path.join(cwd, 'src/lib.rs'), '//! A library.\n')
    fs.writeFileSync(path.join(cwd, 'index.ts'), 'export const value = 1\n')
    childProcess.execFileSync('git', ['add', '.'], { cwd })

    const empty = report(cwd)
    assert.equal(
      empty.metrics.find((metric) => metric.name === 'inline-snapshots')?.score,
      null,
    )

    fs.mkdirSync(path.join(cwd, 'tests'))
    fs.writeFileSync(
      path.join(cwd, 'tests/size.rs'),
      '#[test]\nfn size() { assert_eq!(1 + 1, 2); }\n',
    )
    childProcess.execFileSync('git', ['add', 'tests/size.rs'], { cwd })
    const measured = report(cwd)
    assert.equal(measured.item('inline-snapshots', 'tests/size.rs'), false)
    assert.equal(
      measured.metrics.find((metric) => metric.name === 'inline-snapshots')
        ?.score,
      0,
    )
  })
})
