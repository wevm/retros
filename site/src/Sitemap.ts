/** Lists the site's pages for crawlers. @module */
import * as Manifest from './Manifest.js'

/** Answers `GET /robots.txt`, which allows every crawler and points it to the sitemap. */
export function robots(request: Request): Response {
  const origin = new URL(request.url).origin
  const body = `User-agent: *\nAllow: /\n\nSitemap: ${origin}/sitemap.xml\n`

  return new Response(request.method === 'HEAD' ? null : body, {
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
  })
}

/**
 * Answers `GET /sitemap.xml` with the home page and every documentation page. URLs use the
 * request's origin, so each deployment lists its own pages.
 */
export function respond(request: Request): Response {
  const origin = new URL(request.url).origin
  const paths = [
    '/',
    ...Object.keys(Manifest.pages)
      .sort()
      .map((path) => `/docs/${path}`),
  ]
  const body = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ...paths.map((path) => `  <url><loc>${origin}${path}</loc></url>`),
    '</urlset>',
    '',
  ].join('\n')

  return new Response(request.method === 'HEAD' ? null : body, {
    headers: { 'Content-Type': 'application/xml; charset=utf-8' },
  })
}
