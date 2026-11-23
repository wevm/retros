/** Uses client navigation for site pages and native links for other resources. @module */
import { Link as RouterLink } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

/** Renders a site page link with client navigation, and any other link as an anchor. */
export function Link(props: Link.Props) {
  const { href, target, ...attributes } = props
  const path = href?.split(/[?#]/)[0]

  if (!href || props.download !== undefined) return <a {...props} />

  if (path === '/' || (path?.startsWith('/docs/') && !path.endsWith('.md')))
    return (
      <RouterLink
        {...attributes}
        activeOptions={{ exact: true }}
        href={href}
        to={path}
        {...(target === undefined ? {} : { target })}
      />
    )

  return <a {...props} />
}

export declare namespace Link {
  type Props = ComponentProps<'a'>
}
