/**
 * Measures a website with the tools that already cover each concern. Lighthouse runs its
 * accessibility and best-practices audits on every page of the built site. oxlint's `jsx-a11y`
 * rules read each `.tsx` and `.jsx` file, and stylelint with `@double-great/stylelint-a11y` reads
 * each `.css` file for motion, color scheme, and focus overrides. The script itself only adds the
 * gzip size of each script and stylesheet in the built site, which no tool reports as a metric
 * line. When the project's package.json has a `build` script, the script runs it first.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import * as zlib from 'node:zlib'

import * as ChromeLauncher from 'chrome-launcher'
import lighthouse from 'lighthouse'
import stylelint from 'stylelint'

/** The directory of the built site, which the project's `build` script writes. */
const site = 'dist'

/** The oxlint rules behind `controls`: accessible names, keyboard support, and native elements. */
const controlRules = [
  'alt-text',
  'anchor-has-content',
  'anchor-is-valid',
  'aria-activedescendant-has-tabindex',
  'aria-props',
  'aria-proptypes',
  'aria-role',
  'aria-unsupported-elements',
  'autocomplete-valid',
  'click-events-have-key-events',
  'control-has-associated-label',
  'heading-has-content',
  'html-has-lang',
  'iframe-has-title',
  'img-redundant-alt',
  'interactive-supports-focus',
  'label-has-associated-control',
  'lang',
  'media-has-caption',
  'mouse-events-have-key-events',
  'no-access-key',
  'no-aria-hidden-on-focusable',
  'no-autofocus',
  'no-distracting-elements',
  'no-interactive-element-to-noninteractive-role',
  'no-noninteractive-element-interactions',
  'no-noninteractive-element-to-interactive-role',
  'no-noninteractive-tabindex',
  'no-redundant-roles',
  'no-static-element-interactions',
  'prefer-tag-over-role',
  'role-has-required-aria-props',
  'role-supports-aria-props',
  'scope',
  'tabindex-no-positive',
]

/** The stylelint rule behind each style metric. */
const styleRules = {
  'color-schemes': 'a11y/media-prefers-color-scheme',
  'focus-outlines': 'a11y/no-outline-none',
  'focus-styles': 'a11y/selector-pseudo-class-focus',
  'reduced-motion': 'a11y/media-prefers-reduced-motion',
} as const

const ignored =
  /(^|\/)(\.agents\/packs|dist|build|coverage|node_modules|out|vendor)\//
const lines: string[] = []

build()
await audits()
controls()
await styles()
sizes()

fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Runs the `build` script of the project's package.json, when it has one, and fails with it. */
function build(): void {
  const manifest = fs.existsSync('package.json')
    ? (JSON.parse(fs.readFileSync('package.json', 'utf8')) as build.Manifest)
    : {}
  if (!manifest.scripts?.build) return

  const result = Bun.spawnSync(['bun', 'run', 'build'], {
    stderr: 'inherit',
    stdout: 'inherit',
  })
  if (result.exitCode !== 0) {
    process.stderr.write(`build exited with status ${result.exitCode}\n`)
    process.exit(1)
  }
}

declare namespace build {
  /** The part of package.json that names the build. */
  type Manifest = {
    /** The package's scripts, such as `build`. */
    readonly scripts?: Readonly<Record<string, string>> | undefined
  }
}

/**
 * Writes whether each page passes each Lighthouse audit that counts toward the accessibility and
 * best-practices scores. An audit passes at 0.9 or more, as Lighthouse colors it green.
 */
async function audits(): Promise<void> {
  const pages = fs.existsSync(site)
    ? walk(site).filter((file) => file.endsWith('.html'))
    : []
  if (pages.length === 0) {
    for (const category of ['accessibility', 'best-practices'])
      lines.push(`${category} skip note ${quote(`no HTML pages in ${site}/`)}`)
    return
  }

  const server = Bun.serve({
    fetch: (request) => {
      const pathname = decodeURIComponent(new URL(request.url).pathname)
      const file = path.join(
        site,
        pathname.endsWith('/') ? `${pathname}index.html` : pathname,
      )
      return fs.existsSync(file) && fs.statSync(file).isFile()
        ? new Response(Bun.file(file))
        : new Response('Not found', { status: 404 })
    },
    port: 0,
  })
  const chrome = await ChromeLauncher.launch({
    chromeFlags: ['--headless=new', '--no-sandbox'],
  })

  try {
    for (const page of pages) {
      const url = `http://localhost:${server.port}/${path.relative(site, page)}`
      const result = await lighthouse(url, {
        logLevel: 'error',
        onlyCategories: ['accessibility', 'best-practices'],
        port: chrome.port,
      })
      const report = result!.lhr

      for (const category of ['accessibility', 'best-practices']) {
        for (const reference of report.categories[category]!.auditRefs) {
          const audit = report.audits[reference.id]!
          if (reference.weight === 0 || audit.score === null) continue

          const item = quote(`${path.relative(site, page)} ${reference.id}`)
          const reason = [audit.title, audit.displayValue]
            .filter(Boolean)
            .join('. ')
          lines.push(
            audit.score >= 0.9
              ? `${category} ${item} pass`
              : `${category} ${item} fail note ${quote(reason)}`,
          )
        }
      }
    }
  } finally {
    chrome.kill()
    await server.stop(true)
  }
}

/** Writes whether oxlint's `jsx-a11y` rules find nothing in each `.tsx` and `.jsx` file. */
function controls(): void {
  const files = list(['*.tsx', '*.jsx'])
  if (files.length === 0) {
    lines.push('controls skip note "no .tsx or .jsx files"')
    return
  }

  const linter = bin('oxlint')
  if (!linter) {
    process.stderr.write(
      'Found no oxlint package in node_modules. Install it with `bun add -d oxlint`\n',
    )
    process.exit(1)
  }

  const config = path.join(
    fs.mkdtempSync(path.join(os.tmpdir(), 'uphill-oxlint-')),
    'oxlintrc.json',
  )
  fs.writeFileSync(
    config,
    JSON.stringify({
      categories: {},
      plugins: ['jsx-a11y'],
      rules: Object.fromEntries(
        controlRules.map((rule) => [`jsx-a11y/${rule}`, 'error']),
      ),
    }),
  )

  const found: { file: string; text: string }[] = []
  for (let start = 0; start < files.length; start += 500) {
    const output = Bun.spawnSync([
      linter,
      '-c',
      config,
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
        text: `line ${diagnostic.labels[0]?.span.line ?? 1}: ${diagnostic.message} (${diagnostic.code ?? 'jsx-a11y'})`,
      })
  }

  for (const file of files)
    check({
      item: file,
      metric: 'controls',
      problems: found
        .filter((entry) => entry.file === file)
        .map((entry) => entry.text),
    })
}

/** Writes whether each stylelint rule of the style metrics finds nothing in each `.css` file. */
async function styles(): Promise<void> {
  const files = list(['*.css'])
  if (files.length === 0) {
    for (const metric of Object.keys(styleRules))
      lines.push(`${metric} skip note "no .css files"`)
    return
  }

  // The plugin resolves from the pack's directory, where it is installed, not the project's.
  const result = await stylelint.lint({
    configBasedir: import.meta.dir,
    config: {
      plugins: ['@double-great/stylelint-a11y'],
      rules: Object.fromEntries(
        Object.values(styleRules).map((rule) => [rule, true]),
      ),
    },
    files: [...files],
  })

  for (const [metric, rule] of Object.entries(styleRules))
    for (const file of files) {
      const entry = result.results.find(
        (result) =>
          result.source && path.relative(process.cwd(), result.source) === file,
      )
      const warnings =
        entry?.warnings.filter((warning) => warning.rule === rule) ?? []
      check({
        item: file,
        metric,
        problems: warnings.map(
          (warning) => `line ${warning.line}: ${warning.text}`,
        ),
      })
    }
}

/** Writes the gzip size of each script and stylesheet in the built site. */
function sizes(): void {
  const files = fs.existsSync(site)
    ? walk(site).filter((file) => /\.(css|js|mjs)$/.test(file))
    : []
  if (files.length === 0) {
    lines.push(
      `asset-size skip note ${quote(`no scripts or stylesheets in ${site}/`)}`,
    )
    return
  }

  for (const file of files)
    lines.push(
      `asset-size ${quote(path.relative(site, file))} ${zlib.gzipSync(fs.readFileSync(file), { level: 9 }).length}B`,
    )
}

/** Returns every file under a directory, sorted, so pages and assets keep one order across runs. */
function walk(directory: string): string[] {
  return fs
    .readdirSync(directory, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => path.join(entry.parentPath, entry.name))
    .sort()
}

/** Returns the tracked and untracked files that match the patterns and that Git does not ignore. */
function list(patterns: readonly string[]): string[] {
  const result = Bun.spawnSync([
    'git',
    'ls-files',
    '--cached',
    '--others',
    '--exclude-standard',
    '-z',
    '--',
    ...patterns.flatMap((pattern) => [pattern, `**/${pattern}`]),
  ])
  return [...new Set(result.stdout.toString().split('\0'))]
    .filter((file) => file && fs.existsSync(file) && !ignored.test(file))
    .sort()
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

/**
 * Writes a check for one item: it passes without problems, and fails with the first as its note.
 */
function check(options: check.Options): void {
  const { item, metric, problems } = options
  const more = problems.length > 1 ? ` (and ${problems.length - 1} more)` : ''
  lines.push(
    problems.length === 0
      ? `${metric} ${quote(item)} pass`
      : `${metric} ${quote(item)} fail note ${quote(`${problems[0]}${more}`)}`,
  )
}

/** Quotes an item or note for a metric line, escaping `"` and `\`. */
function quote(text: string): string {
  return `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"').replaceAll('\n', ' ')}"`
}

declare namespace check {
  /** One check: the metric, its item, and the problems that fail it. */
  type Options = {
    item: string
    metric: string
    problems: readonly string[]
  }
}
