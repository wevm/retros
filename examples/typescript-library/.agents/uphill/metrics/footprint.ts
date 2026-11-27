/**
 * Writes what the package costs its consumers: the bytes it publishes, its runtime dependencies,
 * the gzipped size of each entry point and of each export, the bytes an export costs when
 * imported from the root instead of its own entry point, and how many exports the package has.
 * Bun bundles and minifies each import, as a consumer's bundler would.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

const pkg = await Bun.file('package.json').json()
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'footprint-'))
const lines: string[] = []

// `bun pm pack --dry-run` lists every file the package publishes.
const pack = Bun.spawnSync(['bun', 'pm', 'pack', '--dry-run']).stdout.toString()
const files = [...pack.matchAll(/^packed \S+ (.+)$/gm)].map(
  (match) => match[1]!,
)
lines.push(
  `install-size ${files.reduce((total, file) => total + fs.statSync(file).size, 0)}B`,
)
lines.push(`dependencies ${Object.keys(pkg.dependencies ?? {}).length}`)

const entries = Object.entries<string>(pkg.exports).map((pair) => ({
  entry: pair[0],
  file: path.resolve(pair[1]),
}))
const subpaths = entries.filter((entry) => entry.entry !== '.')
const root = entries.find((entry) => entry.entry === '.')!

// The root re-exports every subpath, so it may cost their sum and 10% more, and no more.
const sizes = await Promise.all(subpaths.map((entry) => gzipped(entry.file)))
for (const [index, entry] of subpaths.entries())
  lines.push(`entry-size "${entry.entry}" ${sizes[index]}B`)
const parts = sizes.reduce((total, size) => total + size, 0)
lines.push(
  `entry-size "." ${await gzipped(root.file)}B target ${Math.round(parts * 1.1)}B`,
)

// The root re-exports each subpath as a namespace. Each formatter is a member of one, and the
// formatters keep the code-unit order of a module namespace.
const namespaces = Object.entries<object>(await import(root.file))
const names = namespaces
  .flatMap((pair) =>
    Object.keys(pair[1]).map((name) => ({ name, namespace: pair[0] })),
  )
  .sort((a, b) => (a.name < b.name ? -1 : 1))
lines.push(`exports ${names.length}`)

for (const { name, namespace } of names) {
  const own = await subpathOf(name)
  // Both consumers read the formatter as a namespace member, so only unshaken code differs.
  const fromRoot = await gzipped(
    consumer(`${name}-root.ts`, {
      clause: `{ ${namespace} as formatters }`,
      from: root.file,
      name,
    }),
  )
  const fromOwn = await gzipped(
    consumer(`${name}-own.ts`, {
      clause: '* as formatters',
      from: own.file,
      name,
    }),
  )

  lines.push(`export-size ${name} ${fromRoot}B`)
  lines.push(`unused-bytes ${name} ${Math.max(0, fromRoot - fromOwn)}B`)
}

fs.rmSync(scratch, { force: true, recursive: true })
fs.appendFileSync(process.env.UPHILL_METRICS!, `${lines.join('\n')}\n`)

/** Writes a module that reads one formatter from a namespace import, and returns its path. */
function consumer(file: string, options: consumer.Options): string {
  const out = path.join(scratch, file)
  fs.writeFileSync(
    out,
    `import ${options.clause} from ${JSON.stringify(options.from)}\nexport const ${options.name} = formatters.${options.name}\n`,
  )
  return out
}

/** Returns the subpath entry point that exports `name`. */
async function subpathOf(name: string): Promise<(typeof entries)[number]> {
  for (const entry of subpaths)
    if (name in (await import(entry.file))) return entry
  throw new Error(`no subpath exports ${name}`)
}

/** Bundles and minifies a module, and returns the gzipped size of the output. */
async function gzipped(file: string): Promise<number> {
  const build = await Bun.build({ entrypoints: [file], minify: true })
  return Bun.gzipSync(await build.outputs[0]!.arrayBuffer()).length
}

declare namespace consumer {
  /** The import clause that binds `formatters`, the module to import, and the formatter to read. */
  type Options = {
    clause: string
    from: string
    name: string
  }
}
