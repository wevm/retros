/** Resolves documentation paths to authored MDX pages. @module */
import { createFileRoute, notFound } from '@tanstack/react-router'
import * as Docs from '../Docs.js'
import * as Head from '../Head.js'
import * as Manifest from '../Manifest.js'
import { Page as DocsPage } from '../pages/Docs.js'

export const Route = createFileRoute('/docs/$')({
  component: Page,
  loader: async (entry) => {
    const { params } = entry

    const path = (params._splat ?? '').replace(/\.md$/, '').replace(/\/$/, '')
    if (!Object.hasOwn(Manifest.pages, path)) throw notFound()
    const { description, title } = Manifest.pages[path]!

    // Rendering reads the loaded page synchronously during SSR and client navigation.
    try {
      await Docs.load(path)
    } catch (error) {
      // A deployment can replace the hashed chunk under an open tab, so a full load fetches the current assets.
      if (typeof window !== 'undefined' && entry.cause !== 'preload')
        window.location.assign(entry.location.href)
      throw error
    }

    return { description, path, title }
  },
  head: (entry) => {
    const { loaderData } = entry
    if (!loaderData) return { meta: [{ title: 'Docs · Retros' }] }

    const { description, title } = loaderData
    return Head.page({ description, title: `${title} · Retros` })
  },
})

function Page() {
  return <DocsPage path={Route.useLoaderData().path} />
}
