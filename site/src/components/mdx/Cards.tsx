/** Renders linked cards in a grid, for the More section at the end of a guide. @module */
import type { ReactNode } from 'react'
import { style } from '../../zyzz.config.js'
import { Link } from '../Link.js'

/** Lays out cards in as many columns as fit. */
export function Cards(props: Cards.Props) {
  return <div {...styles.grid()}>{props.children}</div>
}

export declare namespace Cards {
  type Props = { children?: ReactNode }
}

/** Links to a page with a title and a one-line description. */
export function Card(props: Card.Props) {
  const { description, title, to } = props

  return (
    <Link data-card="" href={to} {...styles.card()}>
      <span {...styles.title()}>
        {title} <span aria-hidden="true">→</span>
      </span>
      {description && <span {...styles.description()}>{description}</span>}
    </Link>
  )
}

export declare namespace Card {
  type Props = {
    /** One line below the title. */
    description?: string | undefined
    title: string
    /** The destination, such as `/docs/guides/targets`. */
    to: string
  }
}

namespace styles {
  export const grid = style({
    display: 'grid',
    gap: 3,
    gridTemplateColumns: 'repeat(auto-fill, minmax(14rem, 1fr)) !custom',
    marginBlock: 6,
  })

  export const card = style({
    ':focus-visible': {
      outline: '2px solid',
      outlineColor: 'blue.700',
      outlineOffset: '2px',
    },
    ':hover': { backgroundColor: 'gray.100' },
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

  export const title = style({
    typography: 'label.14',
    fontWeight: '600 !custom',
  })

  export const description = style({ typography: 'copy.13', color: 'gray.900' })
}
