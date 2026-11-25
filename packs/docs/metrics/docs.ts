/**
 * Measures the copy of the project's Markdown against plain technical writing: short sentences and
 * paragraphs, active voice, plain words, no second person, and short headings in one case.
 * textlint reads each file, with published rules where one exists (en-max-word-count, write-good,
 * and pattern) and a few local rules where none does. Each file is an item, and a failing check
 * carries the first problem as its note. Code blocks, inline code, and URLs are not prose, so
 * textlint skips them.
 *
 * @module
 */

import * as fs from 'node:fs'

import pattern from '@textlint-rule/textlint-rule-pattern'
import type { AnyTxtNode } from '@textlint/ast-node-types'
import { TextlintKernel, type TextlintKernelRule } from '@textlint/kernel'
import markdown from '@textlint/textlint-plugin-markdown'
import maxWordCount from 'textlint-rule-en-max-word-count'
import writeGood from 'textlint-rule-write-good'

/**
 * Files that are not documentation, such as generated changelogs, agent guidance, and vendored
 * packs.
 */
const ignored = [
  /(^|\/)\.changeset\//,
  /(^|\/)\.github\//,
  /(^|\/)(AGENTS|CHANGELOG|CLAUDE|LICENSE)(\.md)?$/i,
  /(^|\/)(\.agents\/packs|node_modules)\//,
]

/** `title` for Title Case headings, or `sentence` for sentence case. */
const headingCase: 'sentence' | 'title' = 'title'

/** write-good checks, each turned on alone so that each metric reads one kind. */
const writeGoodOff = {
  adverb: false,
  cliches: false,
  eprime: false,
  illusion: false,
  passive: false,
  so: false,
  thereIs: false,
  tooWordy: false,
  weasel: false,
}

/** Words and marks the conventions ban, each as one textlint pattern. */
const patterns = {
  'conversational-verbs': {
    message: 'Use a precise verb, such as "returns", not a conversational one',
    pattern: '/\\b(hands|gets|gives|sees|knows|says|tells)\\b/i',
  },
  'em-dashes': {
    message: 'Use a comma, a colon, or parentheses, not an em dash',
    pattern: '/—/',
  },
  emoji: {
    message: 'Remove the emoji',
    pattern: '/\\p{Extended_Pictographic}/u',
  },
  'latin-abbreviations': {
    message: 'Write the words, not a Latin abbreviation',
    pattern: '/\\b(e\\.g\\.|i\\.e\\.|etc\\.)/i',
  },
  'second-person': {
    message: 'Write about the code, not the reader',
    pattern: "/\\byou(r|rs|rself|'re|'ll|'ve)?\\b/i",
  },
  semicolons: {
    message: 'Split the sentence, not with a semicolon',
    pattern: '/;/',
  },
  'verb-tenses': {
    message: 'Use a simple tense, not a progressive or perfect one',
    pattern:
      '/\\b(?:is|are|was|were|been|being)\\s+(?!(?:nothing|something|anything|everything|during|string|thing)\\b)[a-z]+ing\\b|\\b(?:has|have|had)\\s+(?:been|[a-z]+ed)\\b/i',
  },
} as const

/** The textlint rules behind each metric. */
const metrics = {
  'conversational-verbs': patternRule('conversational-verbs'),
  'em-dashes': patternRule('em-dashes'),
  emoji: patternRule('emoji'),
  'heading-case': {
    options: {},
    rule: headingRule((text) =>
      cased(text) ? undefined : `"${text}" is not in ${headingCase} case`,
    ),
  },
  'heading-words': {
    options: {},
    rule: headingRule((text) =>
      text.split(/\s+/).filter(Boolean).length >= 4
        ? `"${text}" has 4 or more words`
        : undefined,
    ),
  },
  'latin-abbreviations': patternRule('latin-abbreviations'),
  'paragraph-sentences': {
    options: {},
    rule: paragraphRule((text) =>
      sentences(text) > 6
        ? `${sentences(text)} sentences in one paragraph`
        : undefined,
    ),
  },
  'paragraph-words': {
    options: {},
    rule: paragraphRule((text) =>
      words(text) > 50 ? `${words(text)} words in one paragraph` : undefined,
    ),
  },
  'passive-voice': {
    options: { ...writeGoodOff, passive: true },
    rule: writeGood,
  },
  'second-person': patternRule('second-person'),
  semicolons: patternRule('semicolons'),
  'sentence-words': { options: { max: 25 }, rule: maxWordCount },
  'verb-tenses': patternRule('verb-tenses'),
  'weasel-words': {
    options: { ...writeGoodOff, weasel: true },
    rule: writeGood,
  },
  'wordy-words': {
    options: { ...writeGoodOff, tooWordy: true },
    rule: writeGood,
  },
}

const files = markdownFiles()
if (files.length === 0) {
  process.stderr.write('Found no Markdown files to measure\n')
  process.exit(1)
}

const kernel = new TextlintKernel()
const lines: string[] = []
for (const file of files) {
  const result = await kernel.lintText(fs.readFileSync(file, 'utf8'), {
    ext: '.md',
    filePath: file,
    plugins: [{ plugin: markdown, pluginId: 'markdown' }],
    rules: Object.entries(metrics).map((entry) => ({
      options: entry[1].options,
      rule: entry[1].rule,
      ruleId: entry[0],
    })),
  })
  for (const metric of Object.keys(metrics)) {
    const messages = result.messages.filter(
      (message) => message.ruleId === metric,
    )
    const more = messages.length > 1 ? ` (and ${messages.length - 1} more)` : ''
    const first = messages[0]
    lines.push(
      first
        ? `${metric} ${quote(file)} fail note ${quote(`line ${first.line}: ${first.message}${more}`)}`
        : `${metric} ${quote(file)} pass`,
    )
  }
}
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

function patternRule(name: keyof typeof patterns) {
  // The rule is CommonJS with an `__esModule` default, which Bun unwraps and NodeNext types do not.
  const rule = pattern as unknown as TextlintKernelRule['rule']
  return { options: { patterns: [patterns[name]] }, rule }
}

/**
 * Returns a local textlint rule that reports each paragraph whose text `problem` returns a message
 * for.
 */
function paragraphRule(
  problem: (text: string) => string | undefined,
): TextlintKernelRule['rule'] {
  return (context) => ({
    [context.Syntax.Paragraph!](node: AnyTxtNode) {
      const message = problem(text(node))
      if (message) context.report(node, new context.RuleError(message))
    },
  })
}

/**
 * Returns a local textlint rule that reports each heading whose text `problem` returns a message
 * for.
 */
function headingRule(
  problem: (text: string) => string | undefined,
): TextlintKernelRule['rule'] {
  return (context) => ({
    [context.Syntax.Header!](node: AnyTxtNode) {
      // A heading that is one identifier, such as `target`, names code, not copy.
      if (
        'children' in node &&
        node.children.length === 1 &&
        node.children[0]!.type === 'Code'
      )
        return
      const message = problem(text(node).trim())
      if (message) context.report(node, new context.RuleError(message))
    },
  })
}

/**
 * Returns the prose of a node: its text and the text of its links, with inline code as one word.
 */
function text(node: AnyTxtNode): string {
  if (node.type === 'Code') return 'code'
  if (node.type === 'Str' && 'value' in node) return node.value
  return 'children' in node ? node.children.map(text).join('') : ''
}

function words(prose: string): number {
  return prose.split(/\s+/).filter((word) => /[\p{L}\p{N}]/u.test(word)).length
}

/** Counts sentences, which end at `.`, `?`, or `!` before a capital letter or digit. */
function sentences(prose: string): number {
  return prose
    .replace(/\b(e\.g|i\.e|etc|vs)\./g, '$1')
    .split(/(?<=[.!?])\s+(?=["'(]?[A-Z0-9])/)
    .filter((sentence) => words(sentence) > 0).length
}

/**
 * Returns whether a heading follows the configured case. Words with digits, dots, or inner capitals
 * are names.
 */
function cased(heading: string): boolean {
  const minor = new Set([
    'a',
    'an',
    'and',
    'as',
    'at',
    'but',
    'by',
    'for',
    'from',
    'in',
    'into',
    'nor',
    'of',
    'off',
    'on',
    'onto',
    'or',
    'over',
    'per',
    'so',
    'than',
    'the',
    'to',
    'up',
    'upon',
    'via',
    'with',
    'yet',
  ])
  const list = heading.split(/\s+/).filter(Boolean)
  return list.every((word, index) => {
    const letters = word.replace(/^[^\p{L}]+|[^\p{L}]+$/gu, '')
    if (
      !letters ||
      /[^\p{L}'-]/u.test(letters) ||
      /\p{L}\p{Lu}/u.test(letters) ||
      word === 'code'
    )
      return true
    const upper = /^\p{Lu}/u.test(letters)
    if (index === 0) return upper
    if (headingCase === 'sentence')
      return !upper || letters === letters.toUpperCase()
    if (index === list.length - 1) return upper
    return minor.has(letters.toLowerCase()) ? !upper : upper
  })
}

/** Returns the tracked and untracked Markdown files that Git does not ignore. */
function markdownFiles(): readonly string[] {
  const result = Bun.spawnSync(
    [
      'git',
      'ls-files',
      '--cached',
      '--others',
      '--exclude-standard',
      '-z',
      '--',
      '*.md',
      '*.mdx',
    ],
    { stderr: 'ignore' },
  )
  const found = result.success
    ? result.stdout.toString().split('\0')
    : [...new Bun.Glob('**/*.{md,mdx}').scanSync()]
  return [...new Set(found)]
    .filter(
      (file) =>
        file &&
        fs.existsSync(file) &&
        !ignored.some((pattern) => pattern.test(file)),
    )
    .sort()
}

/** Quotes an item or note for a metric line, escaping `"` and `\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"').replaceAll('\n', ' ')}"`
}
