/** Declares the build-time values that `vite.config.ts` defines. @module */

/** MDX page metadata from the build. Each page chunk carries its own highlighted code. */
declare const __DOCS__: {
  pages: Record<
    string,
    {
      /** The page's opening paragraph. */
      description: string
      /** `##` and `###` headings in document order, with their anchor IDs. */
      headings: readonly { depth: 2 | 3; id: string; title: string }[]
      /** The page's `#` heading. */
      title: string
    }
  >
}

/** Markdown twins from the build, keyed by documentation path. Only server modules read them. */
declare const __MARKDOWN__: Record<string, string>
