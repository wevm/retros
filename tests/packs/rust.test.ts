/** @module Exercises the installed Rust pack against real Cargo workspaces, tools, and compiled targets. */
import * as assert from 'node:assert/strict'
import * as childProcess from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import { test } from 'node:test'
import * as url from 'node:url'

const pack = url.fileURLToPath(
  new URL('../../packs/rust/metrics/rust.ts', import.meta.url),
)
const manifest =
  '[package]\nname = "sample"\nversion = "1.0.0"\nedition = "2024"\n'

function fixture(
  files: Record<string, string>,
  exercise: (root: string) => void,
) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'rust-pack-'))
  try {
    for (const [file, text] of Object.entries(files)) {
      const full = path.join(root, file)
      fs.mkdirSync(path.dirname(full), { recursive: true })
      fs.writeFileSync(full, text)
    }
    childProcess.execFileSync('git', ['init', '--quiet'], { cwd: root })
    exercise(root)
  } finally {
    fs.rmSync(root, { force: true, recursive: true })
  }
}

function measure(root: string) {
  const output = path.join(root, 'measurements')
  fs.writeFileSync(output, '')
  const result = childProcess.spawnSync('bun', [pack], {
    cwd: root,
    encoding: 'utf8',
    env: {
      ...process.env,
      CARGO_NET_OFFLINE: 'true',
      CARGO_TARGET_DIR: path.join(root, 'target'),
      UPHILL_METRICS: output,
    },
    timeout: 90_000,
  })
  if (result.error) throw result.error
  return {
    lines: fs.readFileSync(output, 'utf8').trim().split('\n'),
    status: result.status,
    stderr: result.stderr,
  }
}

test('workspace membership, compiler inputs, dependency identities, and Rust syntax', () => {
  const version = childProcess
    .execFileSync('rustc', ['--version'], { encoding: 'utf8' })
    .split(' ')[1]
  fixture(
    {
      'Cargo.toml': `[workspace]\nmembers = ["app"]\nexclude = ["vendor"]\nresolver = "3"\n[workspace.lints.rust]\nunsafe-code = "deny"\n[profile.release]\nopt-level=3\ncodegen-units=1\nlto=true\nstrip=true\n`,
      'rust-toolchain.toml': `[toolchain]\nchannel = '${version}'\n`,
      'app/Cargo.toml': `${manifest}\n[lints]\nworkspace=true\n[[bin]]\nname="entry"\npath="entry.rs"\n[dependencies]\nold = { package="helper", path="../vendor/v1" }\n[build-dependencies]\nnew = { package="helper", path="../vendor/v2" }\n`,
      'app/build.rs': 'fn main(){ let _ = new::VALUE; }\n',
      'app/entry.rs': '//! Cargo declares this root.\nfn main() {}\n',
      'app/src/lib.rs': `//! Source reachability controls.\n#[path = "../../shared/reached.rs"]\npub mod external;\npub mod outer { pub mod child; }\n// mod ghost;\npub mod clean;\n`,
      'app/src/clean.rs': `   //! These are examples, not code.\n// Result<u8,String>; Rc<RefCell<u8>>; #[test]; mpsc::channel();\n/// Only the generic has two parameters.\npub type Pair = (Result<u8, ()>, u8);\n`,
      'app/src/outer/child.rs': '//! Loaded through an inline module.\n',
      'app/src/ghost.rs': '//! Only a comment mentions this module.\n',
      'shared/reached.rs': `//! Reached from outside the member.\n/// Two-line literal for source offsets.\npub const EXAMPLE: &str = "a\nb";\n/// An actual nested result.\npub type Bad = Result<Vec<u8>, String>;\n/// Caller-visible lint.\npub fn run(){ println!("bad"); let _ = std::sync::mpsc::channel::<Vec<u8>>(); }\n`,
      'app/tests/cases.rs': `//! Real assertion and imported sleep.\nuse std::{thread::sleep, time::Duration};\n#[test]\nfn case(){ let result = "bad".parse::<u8>(); sleep(Duration::ZERO); assert! { result.is_err(), "bad" }; assert_eq! { 4, 4 } }\n`,
      'vendor/v1/Cargo.toml':
        '[package]\nname="helper"\nversion="1.0.0"\nedition="2024"\n',
      'vendor/v1/src/lib.rs': 'pub const VALUE:u8=1;\n',
      'vendor/v2/Cargo.toml':
        '[package]\nname="helper"\nversion="2.0.0"\nedition="2024"\n',
      'vendor/v2/src/lib.rs': 'pub const VALUE:u8=2;\n',
      '.agents/packs/other/example.rs': '// Not project code.\n',
    },
    (root) => {
      childProcess.execFileSync(
        'cargo',
        ['test', '--workspace', '--offline', '--quiet'],
        { cwd: root },
      )
      const run = measure(root)
      assert.equal(run.status, 0, run.stderr)
      for (const expected of [
        'dependencies 2',
        'release-profile pass',
        'toolchain pass',
        'unsafe-denied pass',
        'module-docs "app/src/clean.rs" pass',
        'shared-mutability "app/src/clean.rs" pass',
        'string-errors "app/src/clean.rs" pass',
        'wide-tuples "app/src/clean.rs" pass',
        'orphaned-files "app/entry.rs" pass',
        'orphaned-files "app/src/outer/child.rs" pass',
        'orphaned-files "app/src/ghost.rs" fail note "no workspace target loads this file"',
        'print-macros "shared/reached.rs" fail note "line 8: use of `println!` (clippy::print_stdout)"',
        'string-errors "shared/reached.rs" fail note "line 6: a string as the error"',
        'unbounded-channels "shared/reached.rs" fail note "line 8: a channel without a capacity"',
        'inline-snapshots "app/tests/cases.rs" fail note "2 of 2 assertions are not inline snapshots"',
        'test-sleeps "app/tests/cases.rs" fail note "line 4: a test that sleeps a fixed time"',
        'weak-error-assertions "app/tests/cases.rs" fail note "line 4: an assertion that only checks for an error"',
      ])
        assert.ok(
          run.lines.includes(expected),
          `Expected ${expected}\n${run.lines.filter((line) => line.startsWith(expected.split(' ').slice(0, 2).join(' ') + ' ')).join('\n')}`,
        )
      assert.ok(
        !run.lines.some(
          (line) => line.includes('vendor/') || line.includes('.agents/packs/'),
        ),
      )
    },
  )
})

test('denied diagnostics survive a failed build; syntax and missing inputs never pass', () => {
  fixture(
    {
      'Cargo.toml': `${manifest}\n[workspace]\n`,
      'src/lib.rs':
        '//! A real lint error.\n#![deny(clippy::print_stdout)]\n/// Print.\npub fn emit() { println!("message"); }\n',
      'src/unexamined.rs': '//! No target compiles this file.\n',
    },
    (root) => {
      const denied = measure(root)
      assert.equal(denied.status, 0, denied.stderr)
      assert.ok(
        denied.lines.includes(
          'print-macros "src/lib.rs" fail note "line 4: use of `println!` (clippy::print_stdout)"',
        ),
      )
      assert.ok(
        denied.lines.some((line) =>
          line.startsWith('print-macros "src/unexamined.rs" skip note '),
        ),
      )

      fs.writeFileSync(
        path.join(root, 'src/lib.rs'),
        'pub fn broken() { let = ; }\n',
      )
      const invalid = measure(root)
      assert.equal(invalid.status, 0, invalid.stderr)
      for (const metric of ['clippy', 'formatted', 'unit-tests'])
        assert.ok(
          invalid.lines.some((line) =>
            line.startsWith(`${metric} "src/lib.rs" skip note `),
          ),
          metric,
        )
      assert.ok(
        invalid.lines.some((line) =>
          line.startsWith('dependencies skip note '),
        ),
      )

      fs.appendFileSync(
        path.join(root, 'Cargo.toml'),
        '\n[dependencies]\nabsent = { path="not-present" }\n',
      )
      const missing = measure(root)
      assert.notEqual(missing.status, 0)
      assert.match(missing.stderr, /cargo metadata:.*absent/)
      assert.deepEqual(missing.lines, [''])
    },
  )
})
