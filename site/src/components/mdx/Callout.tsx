/** Renders MDX blockquotes, and labels GitHub alert syntax such as `> [!NOTE]`. @module */
import type { ReactNode } from 'react'
import { style, variants } from '../../zyzz.config.js'

const tones = {
  caution: 'Caution',
  important: 'Important',
  note: 'Note',
  tip: 'Tip',
  warning: 'Warning',
} as const

/** Renders a labeled callout for a marked blockquote, or a plain blockquote otherwise. */
export function Callout(props: Callout.Props) {
  const { children } = props
  const tone = props['data-callout']

  if (!tone || !Object.hasOwn(tones, tone))
    return <blockquote {...styles.quote()}>{children}</blockquote>

  const label = tones[tone]
  return (
    <aside aria-label={label} {...styles.callout({ tone })}>
      <strong {...styles.label()}>{label}</strong>
      <div {...styles.content()}>{children}</div>
    </aside>
  )
}

export declare namespace Callout {
  /** Properties for the Callout component. */
  type Props = {
    children?: ReactNode
    /** The alert type from a leading marker such as `[!NOTE]`. */
    'data-callout'?: keyof typeof tones | undefined
  }
}

namespace styles {
  export const callout = variants({
    base: {
      typography: 'copy.14',
      // The extra element in this selector outranks the article's paragraph margins.
      '& > div > p': { margin: '0 !custom' },
      borderLeft: '3px solid',
      borderRadius: 'md',
      marginBlock: 6,
      paddingBlock: 3,
      paddingInline: 4,
    },
    variants: {
      tone: {
        caution: { backgroundColor: 'red.100', borderColor: 'red.700' },
        important: { backgroundColor: 'purple.100', borderColor: 'purple.700' },
        note: { backgroundColor: 'blue.100', borderColor: 'blue.700' },
        tip: { backgroundColor: 'green.100', borderColor: 'green.700' },
        warning: { backgroundColor: 'amber.100', borderColor: 'amber.700' },
      },
    },
  })

  export const label = style({ display: 'block', marginBottom: 1 })

  export const content = style({ '& > * + *': { marginTop: 2 } })

  export const quote = style({
    borderLeft: '3px solid',
    borderColor: 'gray.500',
    color: 'gray.900',
    marginBlock: 6,
    paddingLeft: 4,
  })
}
