/** Renders a documentation page: the sidebar, the article, the outline, and page links. @module */
import { Suspense, use, useEffect, useId, useState } from 'react'
import { Link } from '../components/Link.js'
import { Callout } from '../components/mdx/Callout.js'
import { Card, Cards } from '../components/mdx/Cards.js'
import { Code } from '../components/mdx/Code.js'
import { Tab, Tabs } from '../components/mdx/Tabs.js'
import { Shell } from '../components/Shell.js'
import * as Docs from '../Docs.js'
import * as Manifest from '../Manifest.js'
import { style } from '../zyzz.config.js'

/** Displays one documentation page in the documentation layout. */
export function Page(props: Page.Props) {
  const { path } = props

  const page = Manifest.pages[path]!
  const sequence = Docs.sequence()
  const position = sequence.indexOf(path)
  const previous = sequence[position - 1]
  const next = sequence[position + 1]

  return (
    <Shell>
      <div {...styles.layout()}>
        <aside {...styles.sidebar()}>
          <Sidebar path={path} />
        </aside>

        {/* Small screens collapse the sidebar above the article. */}
        <details {...styles.menu()}>
          <summary {...styles.menuToggle()}>Contents</summary>
          <Sidebar path={path} />
        </details>

        <article {...styles.article()}>
          <header {...styles.heading()}>
            <p {...styles.eyebrow()}>{Docs.group(path)}</p>
            <h1>{page.title}</h1>
            <p>{page.description}</p>
          </header>

          {/* Hydration keeps the server-rendered page until its chunk loads. */}
          <Suspense>
            <PageContent path={path} />
          </Suspense>

          <nav aria-label="Pagination" {...styles.pagination()}>
            {previous ? (
              <Link href={`/docs/${previous}`} {...styles.pageLink()}>
                <span {...styles.pageDirection()}>Previous</span>
                {Manifest.pages[previous]?.title}
              </Link>
            ) : (
              <span />
            )}
            {next && (
              <Link data-next="" href={`/docs/${next}`} {...styles.pageLink()}>
                <span {...styles.pageDirection()}>Next</span>
                {Manifest.pages[next]?.title}
              </Link>
            )}
          </nav>
        </article>

        <Outline headings={page.headings} path={path} />
      </div>
    </Shell>
  )
}

export declare namespace Page {
  /** Properties for the Page component. */
  type Props = { path: string }
}

/** Renders a page's MDX once its chunk has loaded. */
function PageContent(props: PageContent.Props) {
  const Content = use(Docs.load(props.path))

  return (
    <Content
      components={{
        Card,
        Cards,
        Tab,
        Tabs,
        a: Link,
        blockquote: Callout,
        pre: Code,
      }}
    />
  )
}

declare namespace PageContent {
  type Props = { path: string }
}

/** Lists the page's sections, marks the one scrolled past, and links to the page source. */
function Outline(props: Outline.Props) {
  const { headings, path } = props
  const [current, setCurrent] = useState<string | undefined>(undefined)
  const labelId = useId()

  useEffect(() => {
    const targets = headings.flatMap((heading) => {
      const element = document.getElementById(heading.id)
      return element ? [element] : []
    })
    let frame = 0

    const update = () => {
      frame = 0
      const root = document.documentElement
      // Short final sections never reach the top, so the end of a scrolled page selects the last one.
      const end =
        root.scrollTop > 0 &&
        root.scrollTop + window.innerHeight >= root.scrollHeight - 1
      const passed = end
        ? targets
        : targets.filter(
            (element) =>
              element.getBoundingClientRect().top <=
              Number.parseFloat(getComputedStyle(element).scrollMarginTop) + 1,
          )
      setCurrent(passed.at(-1)?.id)
    }
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update)
    }

    update()
    window.addEventListener('resize', schedule)
    window.addEventListener('scroll', schedule, { passive: true })
    return () => {
      cancelAnimationFrame(frame)
      window.removeEventListener('resize', schedule)
      window.removeEventListener('scroll', schedule)
    }
  }, [headings])

  return (
    <aside {...styles.outline()}>
      {headings.length > 0 && (
        <nav aria-labelledby={labelId}>
          <h2 id={labelId} {...styles.groupHeading()}>
            On This Page
          </h2>
          <ul {...styles.links()}>
            {headings.map((heading) => (
              <li key={heading.id}>
                <a
                  aria-current={heading.id === current ? 'location' : undefined}
                  data-depth={heading.depth}
                  href={`#${heading.id}`}
                  {...styles.outlineLink()}
                >
                  {heading.title}
                </a>
              </li>
            ))}
          </ul>
        </nav>
      )}
      <ul aria-label="Page actions" {...styles.links()}>
        <li>
          <a href={`/docs/${path}.md`} {...styles.outlineLink()}>
            View Markdown
          </a>
        </li>
        <li>
          <a
            href={`https://github.com/wevm/retros/edit/main/site/src/content/docs/${path}.mdx`}
            rel="noreferrer"
            target="_blank"
            {...styles.outlineLink()}
          >
            Edit page
          </a>
        </li>
      </ul>
    </aside>
  )
}

declare namespace Outline {
  type Props = {
    headings: (typeof __DOCS__.pages)[string]['headings']
    path: string
  }
}

/** Lists every page by sidebar group. */
function Sidebar(props: Sidebar.Props) {
  const { path } = props

  return (
    <nav aria-label="Documentation">
      {Docs.groups.map((group) => (
        <div key={group.title} {...styles.group()}>
          <h2 {...styles.groupHeading()}>{group.title}</h2>
          <ul {...styles.links()}>
            {group.pages.map((page) => (
              <li key={page.path}>
                <Link
                  aria-current={page.path === path ? 'page' : undefined}
                  href={`/docs/${page.path}`}
                  {...styles.link()}
                >
                  {page.title}
                </Link>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </nav>
  )
}

declare namespace Sidebar {
  type Props = { path: string }
}

namespace styles {
  export const layout = style({
    alignItems: 'start',
    display: 'grid',
    gap: 10,
    gridTemplateColumns: '13rem minmax(0, 1fr) 12rem !custom',
    marginInline: 'auto !custom',
    maxWidth: '80rem !custom',
    paddingBlock: 10,
    paddingInline: 6,
    '@media (max-width: 1180px)': {
      gridTemplateColumns: '13rem minmax(0, 1fr) !custom',
    },
    '@media (max-width: 860px)': {
      gap: 6,
      gridTemplateColumns: 'minmax(0, 1fr) !custom',
      paddingBlock: 6,
      paddingInline: 4,
    },
  })

  export const sidebar = style({
    maxHeight: 'calc(100dvh - 7rem) !custom',
    overflowY: 'auto',
    position: 'sticky',
    top: 24,
    '@media (max-width: 860px)': { display: 'none' },
  })

  export const menu = style({
    display: 'none',
    '& > nav': { paddingTop: 4 },
    border: '1px solid',
    borderColor: 'gray.400',
    borderRadius: 'lg',
    paddingBlock: 2,
    paddingInline: 3,
    '@media (max-width: 860px)': { display: 'block' },
  })

  export const menuToggle = style({
    typography: 'label.14',
    cursor: 'pointer',
  })

  export const group = style({ marginBottom: 6 })

  export const groupHeading = style({
    typography: 'label.13',
    color: 'foreground',
    fontWeight: '600 !custom',
    marginBottom: 2,
  })

  export const links = style({
    listStyle: 'none',
    margin: '0 !custom',
    padding: '0 !custom',
  })

  export const link = style({
    typography: 'label.14',
    ':focus-visible': {
      outline: '2px solid',
      outlineColor: 'blue.700',
      outlineOffset: '-2px',
    },
    ':hover': { color: 'foreground' },
    '&[aria-current="page"]': {
      backgroundColor: 'gray.200',
      color: 'foreground',
      fontWeight: '500 !custom',
    },
    borderRadius: 'md',
    color: 'gray.900',
    display: 'block',
    paddingBlock: 1.5,
    paddingInline: 2,
    textDecoration: 'none',
  })

  export const article = style({
    typography: 'copy.16',
    '& h2': {
      typography: 'heading.24',
      borderTop: '1px solid',
      borderColor: 'gray.400',
      marginTop: 12,
      paddingTop: 8,
      scrollMarginTop: 24,
    },
    '& h3': {
      typography: 'heading.20',
      marginTop: 8,
      scrollMarginTop: 24,
    },
    '& h4': { typography: 'heading.16', marginTop: 6 },
    '& p, & ul, & ol': { marginBlock: 4 },
    '& ul, & ol': { paddingLeft: 6 },
    '& ul': { listStyleType: 'disc' },
    '& ol': { listStyleType: 'decimal' },
    '& li': { marginBlock: 1.5 },
    // Cards draw their own links.
    '& a:not([data-card])': {
      color: 'blue.900',
      textDecoration: 'underline',
      textUnderlineOffset: '2px !custom',
    },
    '& strong': { fontWeight: '600 !custom' },
    '& :is(p, li, td, th, a, strong, h2, h3, h4) > code': {
      backgroundColor: 'gray.100',
      border: '1px solid',
      borderColor: 'gray.400',
      borderRadius: 'sm',
      fontSize: '0.875em !custom',
      paddingInline: '4px !custom',
    },
    '& table': {
      typography: 'copy.14',
      borderCollapse: 'collapse',
      display: 'block',
      marginBlock: 6,
      overflowX: 'auto',
    },
    '& th, & td': {
      borderBottom: '1px solid',
      borderColor: 'gray.400',
      paddingBlock: 2,
      paddingInline: 3,
      textAlign: 'left',
      verticalAlign: 'top',
    },
    '& th': { fontWeight: '600 !custom' },
    minWidth: '0 !custom',
    overflowWrap: 'anywhere',
  })

  export const heading = style({
    '& h1': { typography: 'heading.40' },
    '& p:last-child': {
      typography: 'copy.18',
      color: 'gray.900',
      marginTop: 3,
    },
    marginBottom: 8,
    '@media (max-width: 640px)': { '& h1': { typography: 'heading.32' } },
  })

  export const eyebrow = style({
    typography: 'label.14',
    color: 'gray.900',
    marginBottom: 2,
  })

  export const pagination = style({
    borderTop: '1px solid',
    borderColor: 'gray.400',
    display: 'grid',
    gap: 4,
    gridTemplateColumns: '1fr 1fr !custom',
    marginTop: 12,
    paddingTop: 8,
  })

  export const pageLink = style({
    typography: 'label.16',
    ':focus-visible': {
      outline: '2px solid',
      outlineColor: 'blue.700',
      outlineOffset: '2px',
    },
    ':hover': { borderColor: 'gray.600' },
    '&[data-next]': { textAlign: 'right' },
    border: '1px solid',
    borderColor: 'gray.400',
    borderRadius: 'lg',
    color: 'foreground',
    display: 'flex',
    flexDirection: 'column',
    gap: 1,
    paddingBlock: 3,
    paddingInline: 4,
    textDecoration: 'none !custom',
  })

  export const pageDirection = style({
    typography: 'label.13',
    color: 'gray.900',
  })

  export const outline = style({
    display: 'flex',
    flexDirection: 'column',
    gap: 6,
    maxHeight: 'calc(100dvh - 7rem) !custom',
    overflowY: 'auto',
    position: 'sticky',
    top: 24,
    '@media (max-width: 1180px)': { display: 'none' },
  })

  export const outlineLink = style({
    typography: 'label.13',
    ':hover': { color: 'foreground' },
    '&[aria-current="location"]': {
      color: 'foreground',
      fontWeight: '500 !custom',
    },
    '&[data-depth="3"]': { paddingLeft: 3 },
    color: 'gray.900',
    display: 'block',
    paddingBlock: 1,
    textDecoration: 'none',
  })
}
