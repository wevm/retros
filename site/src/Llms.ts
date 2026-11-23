/** Lists the documentation for language models in the llms.txt format. @module */
import * as Docs from './Docs.js'
import * as Head from './Head.js'
import * as Manifest from './Manifest.js'
import * as Markdown from './Markdown.js'

const header = ['# Uphill', '', `> ${Head.description}`, '']

/**
 * Answers `/llms.txt` with the site summary and one link per page, grouped as in the sidebar.
 * Links point to the Markdown twins on the request's origin, so each deployment lists its own pages.
 */
export function index(request: Request, options: index.Options = {}): Response {
  const origin = new URL(request.url).origin
  const body = [
    ...header,
    `Each link opens a page's Markdown. [llms-full.txt](${origin}/llms-full.txt) holds every page in one file.`,
    '',
    ...Docs.groups.flatMap((group) => [
      `## ${group.title}`,
      '',
      ...group.pages.flatMap((page) => {
        const entry = Manifest.pages[page.path]
        if (!entry) return []
        return [
          `- [${page.title}](${origin}/docs/${page.path}.md): ${entry.description}`,
        ]
      }),
      '',
    ]),
  ].join('\n')

  return respond(request, body, options.type ?? 'text/plain')
}

export declare namespace index {
  type Options = {
    /** The response media type. Requests for `/` receive the type they negotiated. @default 'text/plain' */
    type?: 'text/markdown' | 'text/plain' | undefined
  }
}

/** Answers `/llms-full.txt` with the site summary and every page's Markdown, in sidebar order. */
export function full(request: Request): Response {
  const origin = new URL(request.url).origin
  const pages = Docs.sequence().map((path) =>
    [
      '---',
      '',
      `Source: ${origin}/docs/${path}`,
      '',
      Markdown.pages[path]?.trim() ?? '',
      '',
    ].join('\n'),
  )

  return respond(request, [...header, ...pages].join('\n'), 'text/plain')
}

/** Sends a text body, and leaves it out of `HEAD` responses. */
function respond(request: Request, body: string, type: string): Response {
  return new Response(request.method === 'HEAD' ? null : body, {
    headers: { 'Content-Type': `${type}; charset=utf-8` },
  })
}
