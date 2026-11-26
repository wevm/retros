/**
 * Builds the site into `dist/` as production serves it: each page at the root, the styles and
 * images beside them, and `/app.js` bundled and minified from `src/app.ts`. The web pack reads
 * `dist/`.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

fs.rmSync('dist', { force: true, recursive: true })
fs.mkdirSync('dist')

for (const page of fs.readdirSync('pages'))
  fs.copyFileSync(path.join('pages', page), path.join('dist', page))
for (const directory of ['images', 'styles'])
  fs.cpSync(directory, path.join('dist', directory), { recursive: true })

const build = await Bun.build({
  entrypoints: ['src/app.ts'],
  minify: true,
  naming: 'app.js',
  outdir: 'dist',
})
if (!build.success) throw new AggregateError(build.logs, 'the build failed')
