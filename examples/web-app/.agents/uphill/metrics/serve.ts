/**
 * Serves the site on a free local port the way production would: gzip for text, a long cache
 * lifetime for assets, and `/app.js` bundled from `src/app.ts`.
 *
 * @module
 */

import * as fs from 'node:fs'
import * as path from 'node:path'

const types: Record<string, string> = {
  '.css': 'text/css',
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.svg': 'image/svg+xml',
}

/** Starts the server and returns it. Pages are at `/` and `/<name>.html`. */
export async function start(): Promise<Bun.Server<undefined>> {
  const build = await Bun.build({ entrypoints: ['src/app.ts'], minify: true })
  const app = new Uint8Array(await build.outputs[0]!.arrayBuffer())

  return Bun.serve({
    port: 0,
    fetch(request) {
      const url = new URL(request.url).pathname
      const file = (() => {
        if (url === '/') return 'pages/index.html'
        if (url.endsWith('.html')) return path.join('pages', url)
        return `.${url}`
      })()
      const body = (() => {
        if (url === '/app.js') return app
        if (fs.existsSync(file)) return fs.readFileSync(file)
        return undefined
      })()
      if (!body) return new Response('Not found', { status: 404 })

      const headers: Record<string, string> = { 'content-type': types[path.extname(url) || '.html']! }
      if (!url.endsWith('.html') && url !== '/') headers['cache-control'] = 'public, max-age=31536000, immutable'
      if (url.endsWith('.svg') || !headers['content-type']!.startsWith('image/')) {
        headers['content-encoding'] = 'gzip'
        return new Response(Bun.gzipSync(body), { headers })
      }
      return new Response(body, { headers })
    },
  })
}
