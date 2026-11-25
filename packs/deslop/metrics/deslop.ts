/**
 * Runs deslop-please (https://github.com/deterministic-machines/deslop-please), the architectural
 * linter for Cargo workspaces, and writes one check per lint for each Rust file in the workspace. A
 * file passes a lint when no diagnostic of that lint points at it, and a failing check carries the
 * first diagnostic as its note. A diagnostic without a location fails the `workspace` item. If
 * `.deslop-please.allow` exists, the tool reads it, so listed debt passes and new errors still
 * fail.
 *
 * @module
 */

import * as fs from 'node:fs'

/**
 * The structural lints of deslop-please, in the order its help lists them. Each lint is a metric.
 * The script ignores `wide-directory` and `long-file`, which measure size rather than structure.
 */
const lints = [
  'module-cycle',
  'tangle',
  'field-leak',
  'shallow-module',
] as const

const allow = '.deslop-please.allow'

const result = Bun.spawnSync(
  ['deslop-please', ...(fs.existsSync(allow) ? ['--allow', allow] : [])],
  {
    env: { ...process.env, NO_COLOR: '1' },
    stderr: 'pipe',
    stdout: 'pipe',
  },
)
const output = `${result.stdout.toString()}${result.stderr.toString()}`

// The tool ends a completed run with `deslop-please: ok` or `deslop-please: N errors`. Any other
// end means it could not index the workspace, so the run fails instead of passing every file.
if (!/^deslop-please: (ok|\d+ errors?\b)/m.test(output)) {
  process.stderr.write(output)
  process.exit(1)
}

const diagnostics = [
  ...output.matchAll(/^error\[([\w-]+)\]: (.*)\n(?:\s+--> (.+?):(\d+):\d+)?/gm),
]
  .map((match) => ({
    file: match[3] ?? 'workspace',
    line: match[4],
    lint: match[1]!,
    text: match[2]!,
  }))
  .filter((diagnostic) =>
    (lints as readonly string[]).includes(diagnostic.lint),
  )

const files = Bun.spawnSync([
  'git',
  'ls-files',
  '--cached',
  '--others',
  '--exclude-standard',
  '--',
  '*.rs',
])
  .stdout.toString()
  .split('\n')
  .filter((file) => file && !/(^|\/)target\//.test(file))
const items = [
  ...new Set([...files, ...diagnostics.map((diagnostic) => diagnostic.file)]),
]

const lines = lints.flatMap((lint) => {
  if (items.length === 0) return [`${lint} skip note "no Rust files"`]

  return items.map((item) => {
    const found = diagnostics.filter(
      (diagnostic) => diagnostic.lint === lint && diagnostic.file === item,
    )
    const first = found[0]
    if (!first) return `${lint} ${quote(item)} pass`

    const more = found.length > 1 ? ` (and ${found.length - 1} more)` : ''
    const location = first.line ? `line ${first.line}: ` : ''
    return `${lint} ${quote(item)} fail note ${quote(`${location}${first.text}${more}`)}`
  })
})

fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Quotes an item or note for a metric line, escaping `"` and `\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`
}
