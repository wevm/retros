/**
 * Answers robots.txt, the sitemap, and the llms.txt files, and serves each documentation page as
 * Markdown at its `.md` path or to clients that ask for Markdown.
 * @module
 */
import {
  createStartHandler,
  defaultStreamHandler,
} from '@tanstack/react-start/server'
import * as Llms from './Llms.js'
import * as Markdown from './Markdown.js'
import * as Sitemap from './Sitemap.js'

const handle = createStartHandler(defaultStreamHandler)

export default {
  async fetch(request: Request, options?: Parameters<typeof handle>[1]) {
    const url = new URL(request.url)
    const read = ['GET', 'HEAD'].includes(request.method)
    if (!read) return handle(request, options)

    if (url.pathname === '/llms.txt') return Llms.index(request)
    if (url.pathname === '/llms-full.txt') return Llms.full(request)
    if (url.pathname === '/robots.txt') return Sitemap.robots(request)
    if (url.pathname === '/sitemap.xml') return Sitemap.respond(request)

    // Agents and terminal clients that ask for Markdown read the documentation index at `/`.
    const type = negotiate(request)
    if (url.pathname === '/' && type) return Llms.index(request, { type })

    if (!url.pathname.startsWith('/docs/')) return handle(request, options)
    const path = url.pathname
      .slice('/docs/'.length)
      .replace(/\.md$/, '')
      .replace(/\/$/, '')
    const markdown = Markdown.pages[path]
    const explicit = url.pathname.endsWith('.md')
    if (markdown === undefined || (!explicit && !type))
      return handle(request, options)

    return new Response(request.method === 'HEAD' ? null : markdown, {
      headers: {
        'Content-Type': `${explicit ? 'text/markdown' : type}; charset=utf-8`,
        Vary: 'Accept',
      },
    })
  },
}

/**
 * Returns the text type a request's `Accept` header names outright and ranks at least as high as
 * HTML, or `undefined` for HTML.
 */
function negotiate(
  request: Request,
): 'text/markdown' | 'text/plain' | undefined {
  const ranges = (request.headers.get('accept') ?? '')
    .split(',')
    .map((range) => {
      const [type, ...parameters] = range.trim().toLowerCase().split(';')
      const q = parameters
        .find((parameter) => parameter.trim().startsWith('q='))
        ?.trim()
        .slice(2)
      return {
        quality: q === undefined ? 1 : Number(q) || 0,
        type: type?.trim(),
      }
    })
  const quality = (type: string) =>
    Math.max(
      0,
      ...ranges
        .filter((range) => range.type === type)
        .map((range) => range.quality),
    )

  return (['text/markdown', 'text/plain'] as const).find(
    (type) => quality(type) > 0 && quality(type) >= quality('text/html'),
  )
}
