/** Lists the documentation pages in sidebar order and loads their compiled MDX. @module */
import type { ComponentType, ElementType, ReactPromise } from 'react'
import * as Manifest from './Manifest.js'

/** Sidebar groups and pages, in reading order: getting started, guides, then reference. */
export const groups: readonly Group[] = [
  {
    title: 'Introduction',
    pages: [
      { path: 'introduction/getting-started', title: 'Getting Started' },
      { path: 'introduction/installation', title: 'Installation' },
    ],
  },
  {
    title: 'Guides',
    pages: [
      { path: 'guides/scripts', title: 'Writing Scripts' },
      { path: 'guides/targets', title: 'Targets' },
      { path: 'guides/baselines', title: 'Baselines' },
      { path: 'guides/comparing', title: 'Comparing Revisions' },
      { path: 'guides/github-actions', title: 'GitHub Actions' },
    ],
  },
  {
    title: 'Reference',
    pages: [
      { path: 'reference/configuration', title: 'Configuration' },
      { path: 'reference/metric-lines', title: 'Metric Lines' },
      { path: 'reference/units', title: 'Units' },
      { path: 'reference/scoring', title: 'Scoring' },
      { path: 'reference/reports', title: 'Reports' },
      { path: 'reference/cli', title: 'CLI' },
      { path: 'reference/action', title: 'GitHub Action' },
    ],
  },
]

// Lazy imports give each page its own chunk.
const modules = import.meta.glob<Content>('./content/docs/**/*.mdx', {
  import: 'default',
})
const loads = new Map<string, ReactPromise<Content>>()

/**
 * Loads a page's compiled MDX once, and marks the promise fulfilled so `use` reads it without
 * suspending again.
 */
export function load(path: string): ReactPromise<Content> {
  const previous = loads.get(path)
  if (previous) return previous

  const module = modules[`./content/docs/${path}.mdx`]
  if (!module) throw new Error(`Missing documentation page: ${path}`)

  const promise: Promise<Content> = module().then(
    (content) => {
      Object.assign(promise, { status: 'fulfilled', value: content })
      return content
    },
    (error: unknown) => {
      // A failed chunk request must not stay cached, so a later attempt can retry it.
      loads.delete(path)
      throw error
    },
  )
  loads.set(path, promise)

  return promise
}

/** Returns the pages in sidebar order that have content, for previous and next links. */
export function sequence(): readonly string[] {
  return groups.flatMap((group) =>
    group.pages
      .map((page) => page.path)
      .filter((path) => Object.hasOwn(Manifest.pages, path)),
  )
}

/** Returns the title of the sidebar group that lists a page, or `undefined`. */
export function group(path: string): string | undefined {
  return groups.find((group) => group.pages.some((page) => page.path === path))
    ?.title
}

/** Compiled MDX component for one documentation page. */
export type Content = ComponentType<{
  /** Elements and MDX components that replace the defaults while rendering. */
  components?: Record<string, ElementType>
}>

/** A sidebar group of pages. */
export type Group = {
  /** Pages in reading order. */
  pages: readonly Page[]
  /** The group heading, such as `Guides`. */
  title: string
}

/** A page in the sidebar. */
export type Page = {
  /** Documentation path, such as `guides/targets`. */
  path: string
  /** Navigation label. */
  title: string
}
