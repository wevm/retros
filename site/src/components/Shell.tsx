/** Frames every page: the header with the site navigation, the content, and the footer. @module */
import type { ReactNode } from 'react'
import { style } from '../zyzz.config.js'
import { Link } from './Link.js'

/** Renders the header, the page, and the footer. */
export function Shell(props: Shell.Props) {
  const { children } = props

  return (
    <div {...styles.page()}>
      <header {...styles.header()}>
        <div {...styles.bar()}>
          <Link aria-label="Uphill home" href="/" {...styles.brand()}>
            <Mark />
            Uphill
          </Link>
          <nav aria-label="Site" {...styles.navigation()}>
            <Link href="/docs/introduction/getting-started" {...styles.link()}>
              Docs
            </Link>
            <a
              href="https://github.com/wevm/uphill"
              rel="noreferrer"
              target="_blank"
              {...styles.link()}
            >
              GitHub
            </a>
          </nav>
        </div>
      </header>

      <main {...styles.main()}>{children}</main>

      <footer {...styles.footer()}>
        <div {...styles.bar()}>
          <span>
            Uphill ·{' '}
            <a href="https://github.com/wevm/uphill/blob/main/LICENSE">
              MIT License
            </a>
          </span>
        </div>
      </footer>
    </div>
  )
}

export declare namespace Shell {
  /** Properties for the Shell component. */
  type Props = { children: ReactNode }
}

/** The Uphill mark: a line that climbs from left to right. */
export function Mark() {
  return (
    <svg aria-hidden="true" viewBox="0 0 32 32" {...styles.mark()}>
      <path d="M4 26 L12 16 L18 21 L28 6" />
    </svg>
  )
}

namespace styles {
  export const page = style({
    display: 'flex',
    flexDirection: 'column',
    minHeight: '100dvh !custom',
  })

  export const header = style({
    backgroundColor: 'background.surface',
    borderBottom: '1px solid',
    borderColor: 'gray.400',
    position: 'sticky',
    top: 0,
    zIndex: 2,
  })

  export const bar = style({
    alignItems: 'center',
    display: 'flex',
    gap: 4,
    justifyContent: 'space-between',
    marginInline: 'auto !custom',
    maxWidth: '80rem !custom',
    paddingInline: 6,
    '@media (max-width: 640px)': { paddingInline: 4 },
  })

  export const brand = style({
    typography: 'heading.20',
    alignItems: 'center',
    color: 'foreground',
    display: 'flex',
    gap: 2,
    height: 16,
    textDecoration: 'none',
  })

  export const mark = style({
    fill: 'none',
    height: 6,
    stroke: 'foreground',
    strokeLinecap: 'round',
    strokeLinejoin: 'round',
    strokeWidth: '3.5 !custom',
    width: 6,
  })

  export const navigation = style({ display: 'flex', gap: 6 })

  export const link = style({
    typography: 'label.14',
    ':focus-visible': {
      outline: '2px solid',
      outlineColor: 'blue.700',
      outlineOffset: '2px',
    },
    ':hover': { color: 'foreground' },
    color: 'gray.900',
    textDecoration: 'none',
  })

  export const main = style({ flex: '1 !custom' })

  export const footer = style({
    typography: 'copy.13',
    '& a': { color: 'gray.900' },
    borderTop: '1px solid',
    borderColor: 'gray.400',
    color: 'gray.900',
    paddingBlock: 6,
  })
}
