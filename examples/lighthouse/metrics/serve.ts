/**
 * Serves the site on a free local port: pages from `pages/`, the page scripts bundled from
 * `src/`, gzip for text, a long cache lifetime for assets, and a `/vendor.js` of about 190 kB
 * gzipped that the home and schedule pages load before they render, as a third-party bundle would.
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
  '.txt': 'text/plain',
}

/** Generated assets, by path. */
const generated: Record<string, string> = {
  '/favicon.svg': '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="8" fill="#0b3954"/></svg>',
  '/llms.txt': '# Harbor\n\n> Ferry timetables, fares, and live departures.\n\n## Pages\n\n- [Schedule](https://harbor.example/schedule.html): today\'s departures\n',
  '/map.svg': `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 320">${Array.from({ length: 400 }, (_, i) => `<circle cx="${(i * 37) % 640}" cy="${(i * 53) % 320}" r="${2 + (i % 5)}" fill="#0b3954"/>`).join('')}</svg>`,
  '/robots.txt': 'User-agent: *\nAllow: /\n',
  // Modules that register themselves and never run, so the bundle costs bytes and parsing. Each
  // holds a key that does not compress, as minified code with embedded data barely does.
  '/vendor.js': Array.from({ length: 6000 }, (_, i) => `window.__m${i}=function(a){return a+"${noise(i, 48)}"};`).join('\n'),
}

/** Returns `length` hexadecimal digits from a fixed linear congruential generator seeded by `seed`. */
function noise(seed: number, length: number): string {
  let state = seed + 1
  let text = ''
  while (text.length < length) {
    state = (state * 1103515245 + 12345) % 2147483648
    text += (state >> 16).toString(16).padStart(4, '0')
  }
  return text.slice(0, length)
}

/** Starts the server and returns it. Pages are at `/` and `/<name>.html`. */
export async function start(): Promise<Bun.Server<undefined>> {
  const scripts: Record<string, Uint8Array> = {}
  for (const name of fs.readdirSync('src')) {
    const build = await Bun.build({ entrypoints: [path.join('src', name)], minify: true })
    scripts[`/${name.replace(/\.ts$/, '.js')}`] = new Uint8Array(await build.outputs[0]!.arrayBuffer())
  }

  return Bun.serve({
    port: 0,
    fetch(request) {
      const url = new URL(request.url).pathname
      const file = url === '/' ? 'pages/index.html' : url.endsWith('.html') ? path.join('pages', url) : `.${url}`
      const body = scripts[url] ?? generated[url] ?? (fs.existsSync(file) && fs.statSync(file).isFile() ? fs.readFileSync(file) : undefined)
      if (!body) return new Response('Not found', { status: 404 })

      const headers: Record<string, string> = {
        'content-encoding': 'gzip',
        'content-type': types[path.extname(url) || '.html']!,
      }
      if (!url.endsWith('.html') && url !== '/') headers['cache-control'] = 'public, max-age=31536000, immutable'
      return new Response(Bun.gzipSync(body), { headers })
    },
  })
}
