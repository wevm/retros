/** Builds the per-page metadata that search engines and link previews read. @module */

/** The site summary, used by the home page and the llms.txt files. */
export const description =
  'Continuous metrics and hill climbing. Retros runs a project’s measurement scripts, scores each metric against its target, and names the fixes worth the most points.'

/** Returns a page's title and description as route `head` tags. */
export function page(options: page.Options) {
  const { description, title } = options

  return {
    meta: [
      { title },
      { content: description, name: 'description' },
      { content: description, property: 'og:description' },
      { content: title, property: 'og:title' },
    ],
  }
}

export declare namespace page {
  type Options = {
    /** Summary for search results and link previews. */
    description: string
    /** Document title, including the site name. */
    title: string
  }
}
