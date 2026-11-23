/**
 * Builds the documentation with TanStack Start for Cloudflare Workers. Reads every MDX page once
 * to collect page metadata and Markdown twins, and highlights code with Shiki in light and dark
 * themes.
 * @module
 */
import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { cloudflare } from '@cloudflare/vite-plugin'
import { compile } from '@mdx-js/mdx'
import mdx from '@mdx-js/rollup'
import {
  transformerMetaWordHighlight,
  transformerNotationHighlight,
} from '@shikijs/transformers'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import react from '@vitejs/plugin-react'
import type { Code, Nodes, Root } from 'mdast'
import { gfmToMarkdown } from 'mdast-util-gfm'
import { defaultHandlers, toMarkdown } from 'mdast-util-to-markdown'
import remarkGfm from 'remark-gfm'
import {
  type BundledLanguage,
  bundledLanguages,
  codeToHast,
  hastToHtml,
} from 'shiki'
import { defineConfig, type ViteDevServer } from 'vite'
import { zyzz } from 'zyzz/vite'

export default defineConfig(async () => {
  const docs: typeof __DOCS__ = { pages: {} }
  const fences: Fences = {}
  const markdowns: typeof __MARKDOWN__ = {}

  const directory = new URL('./src/content/docs/', import.meta.url)
  for (const file of await fs.readdir(directory, { recursive: true })) {
    if (!file.endsWith('.mdx')) continue

    const source = await fs.readFile(new URL(file, directory), 'utf8')
    const page = file
      .replace(/\.mdx$/, '')
      .split(path.sep)
      .join('/')
    await compile(source, {
      remarkPlugins: [remarkGfm, () => (tree: Root) => collect(tree, page)],
    })
  }

  /** Records a page's title, description, headings, highlighted code, and Markdown twin. */
  async function collect(tree: Root, page: string) {
    const heading = tree.children.find(
      (node) => node.type === 'heading' && node.depth === 1,
    )
    const paragraph = tree.children.find((node) => node.type === 'paragraph')
    if (!plain(heading) || !plain(paragraph))
      throw new Error(
        `Documentation page ${page} needs a title and a subtitle.`,
      )

    const highlighted = new Map<Code, Element>()
    async function highlight(node: Nodes) {
      if ('children' in node)
        for (const child of node.children) await highlight(child)
      if (node.type !== 'code') return

      const root = await codeToHast(node.value, {
        defaultColor: 'light-dark()',
        lang:
          node.lang && Object.hasOwn(bundledLanguages, node.lang)
            ? (node.lang as BundledLanguage)
            : 'text',
        // Titles can contain paths, whose slashes would read as `/word/` highlights.
        meta: { __raw: node.meta?.replace(titleMeta, '') ?? '' },
        themes: { dark: 'github-dark-default', light: 'github-light-default' },
        transformers: [
          transformerNotationHighlight({ matchAlgorithm: 'v3' }),
          transformerMetaWordHighlight(),
        ],
      })
      const pre = root.children[0]
      const code = pre?.type === 'element' ? pre.children[0] : undefined
      if (code?.type !== 'element')
        throw new Error(`Shiki returned no code element in ${page}.`)

      highlighted.set(node, code)
      fences[node.value] = {
        html: hastToHtml({ type: 'root', children: code.children }),
        text: lines(code).join('\n'),
      }
    }
    await highlight(tree)

    docs.pages[page] = {
      description: plain(paragraph),
      headings: headings(tree),
      title: plain(heading),
    }
    markdowns[page] = toMarkdown(tree, {
      extensions: [gfmToMarkdown()],
      handlers: {
        code: (node, parent, state, info) => {
          const code = highlighted.get(node)
          return defaultHandlers.code(
            code ? { ...node, value: lines(code).join('\n') } : node,
            parent,
            state,
            info,
          )
        },
        mdxjsEsm: () => '',
        mdxFlowExpression: () => '',
        mdxTextExpression: () => '',
        mdxJsxFlowElement: (node, _parent, state, info) => {
          const attribute = (name: string) => {
            const entry = node.attributes.find(
              (attribute: { name?: string; type: string }) =>
                attribute.type === 'mdxJsxAttribute' && attribute.name === name,
            )
            return entry?.type === 'mdxJsxAttribute' &&
              typeof entry.value === 'string'
              ? entry.value
              : undefined
          }
          const content = state.containerFlow(node, info)

          if (node.name === 'Card') {
            const title = attribute('title')
            const to = attribute('to')
            if (title === undefined || to === undefined) return content
            const description = attribute('description')
            return `- [${title}](${to})${description ? `: ${description}` : ''}`
          }
          if (node.name === 'Tab')
            return `#### ${attribute('title') ?? ''}\n\n${content}`
          return content
        },
        mdxJsxTextElement: (node, _parent, state, info) =>
          state.containerPhrasing(node, info),
      },
    })
      .trimStart()
      .replace(escapedAlert, '$1[!$2]')
  }

  return {
    define: {
      __DOCS__: JSON.stringify(docs),
      __MARKDOWN__: JSON.stringify(markdowns),
    },
    plugins: [
      cloudflare({ viteEnvironment: { name: 'ssr' } }),
      zyzz(),
      mdx({
        remarkPlugins: [
          remarkGfm,
          () => (tree: Root) => {
            annotate(tree)

            // The shared page header renders the authored title and opening paragraph.
            const heading = tree.children.find(
              (node) => node.type === 'heading' && node.depth === 1,
            )
            const paragraph = tree.children.find(
              (node) => node.type === 'paragraph',
            )
            tree.children = tree.children.filter(
              (node) => node !== heading && node !== paragraph,
            )

            // Assigns the anchor IDs that `__DOCS__` headings link to.
            headings(tree)
          },
        ],
      }),
      {
        name: 'docs-content',
        configureServer(server: ViteDevServer) {
          const content = path.resolve('src/content/docs')
          server.watcher.add(content)
          let timer: ReturnType<typeof setTimeout> | undefined
          server.watcher.on('all', (_event, file) => {
            if (!file.startsWith(`${content}${path.sep}`)) return
            clearTimeout(timer)
            // The metadata pass runs once per server, so content edits restart it.
            timer = setTimeout(() => void server.restart(), 100)
          })
          server.httpServer?.on('close', () => clearTimeout(timer))
        },
      },
      tanstackStart({ server: { entry: './entry.server.ts' } }),
      react(),
    ],
  }

  /** Marks GitHub alerts on blockquotes, and attaches highlighted markup to code fences. */
  function annotate(node: Nodes) {
    if ('children' in node) for (const child of node.children) annotate(child)

    if (node.type === 'blockquote') {
      const paragraph = node.children[0]
      if (paragraph?.type !== 'paragraph') return
      const marker = paragraph.children[0]
      if (marker?.type !== 'text') return
      const match = marker.value.match(calloutMarker)
      if (!match) return

      marker.value = marker.value.slice(match[0].length)
      if (!marker.value) paragraph.children.shift()
      if (!paragraph.children.length) node.children.shift()
      node.data = {
        ...node.data,
        hProperties: {
          ...node.data?.hProperties,
          'data-callout': match[1]?.toLowerCase(),
        },
      }
      return
    }

    if (node.type !== 'code') return
    const highlighted = fences[node.value]
    // The page chunk carries its own highlighted markup, so browsers load no highlighter.
    node.data = {
      ...node.data,
      hProperties: {
        ...node.data?.hProperties,
        'data-filename': node.meta?.match(titleAttribute)?.[1],
        'data-html': highlighted?.html,
        'data-text': highlighted?.text,
      },
    }
  }
})

/** Matches a GitHub alert marker, such as `[!NOTE]`, at the start of a blockquote. */
const calloutMarker = /^\[!(CAUTION|IMPORTANT|NOTE|TIP|WARNING)\]\s*/

/** Matches an alert marker that Markdown serialization escaped, which GitHub alerts need literally. */
const escapedAlert = /^((?:> ?)+)\\\[!(CAUTION|IMPORTANT|NOTE|TIP|WARNING)\]/gm

/** Matches a `title="..."` attribute in a code fence's metadata, and captures its value. */
const titleAttribute = /(?:^|\s)title="([^"]+)"/

/** Matches a `title="..."` attribute in a code fence's metadata. */
const titleMeta = /(?:^|\s)title="[^"]*"/

/** Highlighted code fences, keyed by authored source. */
type Fences = Record<
  string,
  {
    /** Shiki markup for the lines inside `<code>`, including notation highlights. */
    html: string
    /** Displayed source without notation, which the copy button copies. */
    text: string
  }
>

/** A highlighted HAST element from Shiki. */
type Element = Extract<
  Awaited<ReturnType<typeof codeToHast>>['children'][number],
  { type: 'element' }
>

/**
 * Assigns anchor IDs to `##` and `###` headings and returns them in document order. Both MDX
 * passes call it, so the IDs match.
 */
function headings(tree: Root) {
  const counts = new Map<string, number>()

  function visit(node: Nodes): (typeof __DOCS__.pages)[string]['headings'] {
    // Tabs show one panel at a time, so their headings label panels rather than sections.
    if (node.type === 'mdxJsxFlowElement' && node.name === 'Tabs') return []
    if (node.type !== 'heading')
      return 'children' in node ? node.children.flatMap(visit) : []
    if (node.depth !== 2 && node.depth !== 3) return []

    const title = plain(node)
    const slug = title
      .toLowerCase()
      .replace(/[^\p{L}\p{N}\s-]/gu, '')
      .trim()
      .replace(/\s+/g, '-')
    const count = counts.get(slug) ?? 0
    counts.set(slug, count + 1)
    const id = count ? `${slug}-${count}` : slug

    node.data = { ...node.data, hProperties: { ...node.data?.hProperties, id } }
    return [{ depth: node.depth, id, title }]
  }

  return visit(tree)
}

/** Returns the displayed lines of highlighted code. */
function lines(code: Element): string[] {
  return code.children.flatMap((child) =>
    child.type === 'element' ? [text(child)] : [],
  )
}

/** Returns a Markdown node's plain text, such as a heading's title. */
function plain(node: Nodes | undefined): string {
  if (!node) return ''
  if ('children' in node) return node.children.map(plain).join('')
  if ('value' in node) return node.value
  return ''
}

/** Returns the displayed text of a HAST node. */
function text(node: Element['children'][number]): string {
  if (node.type === 'text') return node.value
  if (node.type !== 'element') return ''
  return node.children.map(text).join('')
}
