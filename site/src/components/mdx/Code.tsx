/** Renders MDX code fences with build-time highlighting and a copy button. @module */
import { isValidElement, type ReactNode, useEffect, useState } from 'react'
import { style } from '../../zyzz.config.js'

/** Renders a `<pre>` from MDX as a code panel with its file name or language. */
export function Code(props: Code.Props) {
  const { children } = props
  const [copied, setCopied] = useState<'idle' | 'copied' | 'failed'>('idle')

  useEffect(() => {
    if (copied === 'idle') return
    const timer = window.setTimeout(() => setCopied('idle'), 2000)
    return () => window.clearTimeout(timer)
  }, [copied])

  if (!isValidElement<Code.Element>(children))
    return <pre {...styles.code()}>{children}</pre>

  const filename = children.props['data-filename']
  const html = children.props['data-html']
  const source = children.props.children?.replace(/\n$/, '') ?? ''
  const text = children.props['data-text'] ?? source
  const language = /language-([\w-]+)/.exec(children.props.className ?? '')?.[1]

  return (
    <div {...styles.block()}>
      <div {...styles.header()}>
        <span>{filename ?? language ?? 'text'}</span>
        <button
          onClick={async () => {
            try {
              await navigator.clipboard.writeText(text)
              setCopied('copied')
            } catch {
              setCopied('failed')
            }
          }}
          type="button"
          {...styles.copy()}
        >
          <span aria-live="polite">
            {{ copied: 'Copied', failed: 'Copy failed', idle: 'Copy' }[copied]}
          </span>
        </button>
      </div>
      <pre {...styles.code()}>
        {html ? (
          // Shiki highlights the lines during the build.
          <code dangerouslySetInnerHTML={{ __html: html }} />
        ) : (
          <code>{source}</code>
        )}
      </pre>
    </div>
  )
}

export declare namespace Code {
  /** Properties for the Code component. */
  type Props = { children?: ReactNode }

  /** Properties of the `<code>` element that MDX renders inside `<pre>`, with the build's annotations. */
  type Element = {
    children?: string
    className?: string | undefined
    'data-filename'?: string | undefined
    'data-html'?: string | undefined
    'data-text'?: string | undefined
  }
}

namespace styles {
  export const block = style({
    border: '1px solid',
    borderColor: 'gray.400',
    borderRadius: 'lg',
    marginBlock: 6,
    overflow: 'hidden',
  })

  export const header = style({
    typography: 'label.12',
    alignItems: 'center',
    backgroundColor: 'gray.100',
    borderBottom: '1px solid',
    borderColor: 'gray.400',
    color: 'gray.900',
    display: 'flex',
    fontFamily: 'mono',
    justifyContent: 'space-between',
    paddingBlock: 1.5,
    paddingInline: 4,
  })

  export const copy = style({
    ':focus-visible': {
      outline: '2px solid',
      outlineColor: 'blue.700',
      outlineOffset: '2px',
    },
    ':hover': { color: 'foreground' },
    backgroundColor: 'transparent !custom',
    color: 'inherit !custom',
    cursor: 'pointer',
    font: 'inherit',
  })

  export const code = style({
    '& .highlighted': {
      backgroundColor: 'grayAlpha.200',
      display: 'inline-block',
      minWidth: '100% !custom',
    },
    '& .highlighted-word': {
      backgroundColor: 'grayAlpha.300',
      borderRadius: 'xs',
    },
    '& code': {
      display: 'block',
      font: 'inherit',
      minWidth: 'max-content !custom',
    },
    backgroundColor: 'background.primary',
    fontFamily: 'mono',
    fontSize: '13px !custom',
    lineHeight: '1.7 !custom',
    margin: '0 !custom',
    overflowX: 'auto',
    paddingBlock: 4,
    paddingInline: 4,
  })
}
