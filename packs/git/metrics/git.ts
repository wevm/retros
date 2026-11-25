/**
 * Measures the project's Git history and repository hygiene. commitlint reads the subject of each
 * recent commit with the conventional rules, and `git diff --check` reads whitespace. Local checks
 * cover what no tool reads: reasons in subjects, imperative subjects, bug fixes without a test,
 * workflow key order, changeset sentences, and tracked build output. Each commit or
 * file is an item.
 *
 * @module
 */

import * as fs from 'node:fs'

import conventional from '@commitlint/config-conventional'
import lint from '@commitlint/lint'
import createPreset from 'conventional-changelog-conventionalcommits'

/** Commits to read, from `HEAD` back. */
const depth = Number(process.env.UPHILL_GIT_COMMITS ?? 50)

/** commitlint rules, by the metric that reads them. */
const groups = {
  'commit-type': [
    'type-case',
    'type-empty',
    'type-enum',
    'subject-empty',
    'header-trim',
  ],
  'header-length': ['header-max-length'],
  'message-body': [
    'body-leading-blank',
    'body-max-line-length',
    'footer-leading-blank',
    'footer-max-line-length',
  ],
  'subject-case': ['subject-case'],
  'subject-full-stop': ['subject-full-stop'],
} as const

/** Directories of build output and installed packages, which Git should never track. */
const output =
  /(^|\/)(node_modules|dist|target|coverage|zig-out|\.zig-cache|\.next|\.turbo)\/|(^|\/)\.env(\.[\w.-]+)?$/

const lines: string[] = []

if (
  !Bun.spawnSync(['git', 'rev-parse', '--verify', 'HEAD'], { stderr: 'ignore' })
    .success
) {
  process.stderr.write('Found no Git commits to measure\n')
  process.exit(1)
}

await commits()
hygiene()
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Writes commitlint's verdict on each recent commit, and the checks commitlint has no rule for. */
async function commits(): Promise<void> {
  // `%x00` ends each message and `%x01` separates the hash from it, since messages hold newlines.
  const log = run([
    'git',
    'log',
    '--no-merges',
    `-n${depth}`,
    '--format=%h%x01%B%x00',
  ])
  const entries = log
    .split('\0')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const [hash = '', message = ''] = entry.split('\x01')
      return { hash, message: message.trim() }
    })
  // The conventional commits parser reads `!` before the colon as a breaking change.
  // The preset has no types, so its parser takes the type that `lint` reads.
  const preset = (await createPreset()) as {
    parser: NonNullable<NonNullable<Parameters<typeof lint>[2]>['parserOpts']>
  }
  const parserOpts = preset.parser
  let fixes = 0

  for (const entry of entries) {
    const result = await lint(entry.message, conventional.rules, { parserOpts })
    for (const [metric, names] of Object.entries(groups)) {
      const errors = result.errors.filter((error) =>
        (names as readonly string[]).includes(error.name),
      )
      check({
        item: entry.hash,
        metric,
        problems: errors.map(
          (error) => `${error.message}: ${subject(entry.message)}`,
        ),
      })
    }

    const header = entry.message.split('\n')[0] ?? ''
    const description = header.replace(/^\w+(\([^)]*\))?!?:\s*/, '')
    check({
      item: entry.hash,
      metric: 'subject-reasons',
      problems: /\b(because|so that|in order to)\b/i.test(description)
        ? [`states a reason: ${header}`]
        : [],
    })
    const verb = description.split(/\s/)[0] ?? ''
    check({
      item: entry.hash,
      metric: 'imperative-subjects',
      problems: /^[a-z]+(ed|[^s]s)$/.test(verb)
        ? [`"${verb}" is not imperative: ${header}`]
        : [],
    })

    if (/^fix(\(|!|:)/.test(header)) {
      fixes++
      const changed = run([
        'git',
        'show',
        '--name-only',
        '--format=',
        entry.hash,
      ]).split('\n')
      const tested = changed.some((file) =>
        /(^|\/)(tests?|__tests__|spec)\/|[._](test|spec)\.\w+$|_test\.\w+$/.test(
          file,
        ),
      )
      check({
        item: entry.hash,
        metric: 'fix-tests',
        problems: tested ? [] : [`changes no test: ${header}`],
      })
    }
  }
  if (fixes === 0)
    lines.push(
      `fix-tests skip note ${quote(`no fix: commits in the last ${depth}`)}`,
    )
}

/** Writes whitespace errors, workflow key order, changeset sentences, and tracked build output. */
function hygiene(): void {
  const tracked = run(['git', 'ls-files', '-z']).split('\0').filter(Boolean)

  // `git diff --check` against the empty tree reads every tracked line for whitespace errors.
  const empty = run(['git', 'hash-object', '-t', 'tree', '/dev/null']).trim()
  const errors = run(['git', 'diff', '--check', empty, 'HEAD'])
    .split('\n')
    .map((line) => /^(.+?):(\d+): (.+)$/.exec(line))
    .filter((match) => match !== null)
  // `git ls-files --eol` marks binary files `i/-text`.
  const binary = new Set(
    run(['git', 'ls-files', '--eol', '-z'])
      .split('\0')
      .filter((entry) => entry.startsWith('i/-text'))
      .map((entry) => entry.split('\t')[1]),
  )
  for (const file of tracked.filter(
    (file) =>
      !/\.(md|mdx|patch|diff|snap)$/.test(file) &&
      fs.existsSync(file) &&
      !binary.has(file),
  )) {
    const error = errors.find((match) => match[1] === file)
    check({
      item: file,
      metric: 'whitespace',
      problems: error ? [`line ${error[2]}: ${error[3]}`] : [],
    })
  }

  const workflows = tracked.filter((file) =>
    /^\.github\/workflows\/[^/]+\.ya?ml$/.test(file),
  )
  for (const file of workflows) {
    const keys = fs
      .readFileSync(file, 'utf8')
      .split('\n')
      .map((line) => /^([\w-]+):/.exec(line)?.[1])
      .filter((key) => key !== undefined)
    const rest = keys.filter((key) => key !== 'name' && key !== 'on')
    const expected = [
      ...keys.filter((key) => key === 'name' || key === 'on'),
      ...[...rest].sort(),
    ]
    const first = keys[0] === 'name' && keys[1] === 'on'
    check({
      item: file,
      metric: 'workflow-keys',
      problems:
        first && keys.join() === expected.join()
          ? []
          : [
              `top-level keys are ${keys.join(', ')}, not name, on, then alphabetical`,
            ],
    })
  }
  if (workflows.length === 0)
    lines.push('workflow-keys skip note "no workflows"')

  const changesets = tracked.filter(
    (file) =>
      /^\.changeset\/[^/]+\.md$/.test(file) && !file.endsWith('README.md'),
  )
  for (const file of changesets) {
    const body = fs
      .readFileSync(file, 'utf8')
      .replace(/^---[\s\S]*?---/, '')
      .replace(/```[\s\S]*?```/g, '')
      .trim()
    const sentences = body.split(/(?<=[.!?])\s+(?=[A-Z`])/).filter(Boolean)
    const verb = body.replace(/^[^A-Za-z]+/, '').split(/\s/)[0] ?? ''
    check({
      item: file,
      metric: 'changeset-sentences',
      problems: [
        ...(sentences.length > 1
          ? [`${sentences.length} sentences, not one`]
          : []),
        ...(/^[A-Z][a-z]*(ed|ade|ote|ilt|ent|ept|ound|un)$/.test(verb)
          ? []
          : [`"${verb}" is not a past-tense verb`]),
      ],
    })
  }
  if (changesets.length === 0)
    lines.push('changeset-sentences skip note "no changesets"')

  check({
    metric: 'tracked-output',
    problems: tracked
      .filter((file) => output.test(file))
      .map((file) => `Git tracks ${file}`),
  })
}

function subject(message: string): string {
  return message.split('\n')[0] ?? ''
}

/** Runs a command and returns its standard output. A failing exit status is a measurement. */
function run(command: readonly string[]): string {
  return Bun.spawnSync([...command], { stderr: 'ignore' }).stdout.toString()
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
      : `${subject} fail note ${quote(`${problems[0]}${more}`)}`,
  )
}

/** Quotes an item or note for a metric line, escaping `"` and `\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"').replaceAll('\n', ' ')}"`
}

declare namespace check {
  /** One check: the metric, its item, and the problems that fail it. */
  type Options = {
    item?: string | undefined
    metric: string
    problems: readonly string[]
  }
}
