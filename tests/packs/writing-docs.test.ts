/** @module Runs the writing pack against Markdown in an independent Git checkout. */
import * as assert from 'node:assert/strict'
import * as child from 'node:child_process'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(
  new URL('../../packs/writing-docs/metrics/writing-docs.ts', import.meta.url),
)

function measure(files: Record<string, string>) {
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-writing-'))
  const output = path.join(cwd, 'metrics')

  try {
    child.execFileSync('git', ['init', '--quiet'], { cwd })
    for (const [file, source] of Object.entries(files))
      fs.writeFileSync(path.join(cwd, `${file}.md`), source)

    child.execFileSync('bun', [script], {
      cwd,
      env: { ...process.env, UPHILL_METRICS: output },
      timeout: 30_000,
    })
    return fs.readFileSync(output, 'utf8').trimEnd().split('\n')
  } finally {
    fs.rmSync(cwd, { force: true, recursive: true })
  }
}

test('checks rendered prose across markup without checking destinations, code, or comments', () => {
  const lines = measure({
    clean: [
      '# npm Packages',
      'The record is missing. Both hands move. The tool has a record.',
      '[format](https://example.com/your/input;a) <https://example.com/your/input;a> https://example.com/your/input;a',
      '[Open **https://example.com/your/input;a** here](https://example.org)',
      '`you; 🇦🇺 has run`',
      '<!-- you; 🚀 -->',
      '```txt\nyour code; has finished\n```',
    ].join('\n\n'),
    conversation: 'Does the parser give results?',
    flag: 'Locale: 🇦🇺.',
    interrupted: 'The task has `work` finished. In `sort` order to proceed.',
    irregular: 'The task has run.',
    keycap: 'Key: 1️⃣.',
    label: '[**Your** choices;](https://example.com)',
    major: '## Parse Or Write',
    minor: '## Parse or Write',
    npm: '## npm',
    passive: 'The data *is* processed.',
    productCase: '## Npm Packages',
    progressive: 'The tool is measuring size.',
    regular: 'The task has finished.',
    rocket: 'Locale: 🚀.',
    wordy: 'In **order** to parse, load the file.',
    wrapped:
      '## Checks\n\nThe parser reads the header.\nThen the parser **has** finished.',
  })

  const clean = lines.filter((line) => line.includes(' "clean.md" '))
  assert.equal(clean.length, 15)
  assert.deepEqual(
    clean.filter((line) => !line.endsWith(' pass')),
    [],
  )

  for (const [metric, file] of [
    ['conversational-verbs', 'conversation'],
    ['emoji', 'flag'],
    ['emoji', 'keycap'],
    ['emoji', 'rocket'],
    ['heading-case', 'major'],
    ['heading-case', 'productCase'],
    ['passive-voice', 'passive'],
    ['second-person', 'label'],
    ['semicolons', 'label'],
    ['verb-tenses', 'irregular'],
    ['verb-tenses', 'progressive'],
    ['verb-tenses', 'regular'],
    ['wordy-words', 'wordy'],
  ]) {
    const prefix = `${metric} "${file}.md" fail note "line 1:`
    assert.ok(
      lines.some((line) => line.startsWith(prefix)),
      prefix,
    )
  }

  assert.match(
    lines.find((line) => line.startsWith('verb-tenses "wrapped.md"')) ?? '',
    /fail note "line 4: .*has finished/,
  )
  for (const expected of [
    'heading-case "minor.md" pass',
    'heading-case "npm.md" pass',
    'verb-tenses "interrupted.md" pass',
    'wordy-words "interrupted.md" pass',
  ])
    assert.ok(lines.includes(expected), expected)
})

test('reports word and sentence limits at their exact boundaries', () => {
  const words = (count: number) => Array(count).fill('word').join(' ')
  const six = Array(6).fill('npm exits.').join(' ')
  const lines = measure({
    heading3: '## Three Short Words',
    heading4: '## Four Very Short Words',
    paragraph50: `${words(25)}. ${words(25)}.`,
    paragraph51: `${words(25)}. ${words(26)}.`,
    sentence25: `${words(24)} **word** \`ignored words\` <https://example.org/ignored>.`,
    sentence26: `npm exits.\n${words(25)} [word](https://example.org/ignored).`,
    seven: `${six}\nnpm exits.`,
    six,
  })

  for (const expected of [
    'heading-words "heading3.md" pass',
    'paragraph-sentences "six.md" pass',
    'paragraph-words "paragraph50.md" pass',
    'sentence-words "paragraph50.md" pass',
    'sentence-words "sentence25.md" pass',
  ])
    assert.ok(lines.includes(expected), expected)

  for (const [prefix, evidence] of [
    ['heading-words "heading4.md"', /line 1: .*4 words.*limit 3/],
    [
      'paragraph-sentences "seven.md"',
      /line 2: .*7 sentences.*limit 6.*npm exits/,
    ],
    ['paragraph-words "paragraph51.md"', /line 1: .*51 words.*limit 50/],
    [
      'sentence-words "sentence26.md"',
      /line 2: .*26 words.*limit 25.*word word/,
    ],
  ] as const)
    assert.match(lines.find((line) => line.startsWith(prefix)) ?? '', evidence)
})
