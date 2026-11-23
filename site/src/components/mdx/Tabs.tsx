/** Shows one of several panels, such as the same script in shell, TypeScript, and Rust. @module */
import {
  Children,
  isValidElement,
  type ReactNode,
  useId,
  useState,
} from 'react'
import { style } from '../../zyzz.config.js'

/** Renders a row of tabs above the selected panel. Every panel stays in the page for search engines. */
export function Tabs(props: Tabs.Props) {
  const tabs = Children.toArray(props.children).filter((child) =>
    isValidElement<Tab.Props>(child),
  )
  const [selected, setSelected] = useState(0)
  const id = useId()

  return (
    <div {...styles.tabs()}>
      <div aria-label="Variants" role="tablist" {...styles.list()}>
        {tabs.map((tab, index) => (
          <button
            aria-controls={`${id}-${index}-panel`}
            aria-selected={index === selected}
            id={`${id}-${index}`}
            key={tab.props.title}
            onClick={() => setSelected(index)}
            onKeyDown={(event) => {
              if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft')
                return
              const step = event.key === 'ArrowRight' ? 1 : -1
              const next = (selected + step + tabs.length) % tabs.length
              setSelected(next)
              document.getElementById(`${id}-${next}`)?.focus()
            }}
            role="tab"
            tabIndex={index === selected ? 0 : -1}
            type="button"
            {...styles.tab()}
          >
            {tab.props.title}
          </button>
        ))}
      </div>
      {tabs.map((tab, index) => (
        <div
          aria-labelledby={`${id}-${index}`}
          hidden={index !== selected}
          id={`${id}-${index}-panel`}
          key={tab.props.title}
          role="tabpanel"
        >
          {tab.props.children}
        </div>
      ))}
    </div>
  )
}

export declare namespace Tabs {
  type Props = { children?: ReactNode }
}

/** One panel of `Tabs`, labeled by `title`. */
export function Tab(props: Tab.Props) {
  return <>{props.children}</>
}

export declare namespace Tab {
  type Props = { children?: ReactNode; title: string }
}

namespace styles {
  export const tabs = style({
    marginBlock: 6,
    '& [role="tabpanel"] > :first-child': { marginTop: '0 !custom' },
  })

  export const list = style({
    borderBottom: '1px solid',
    borderColor: 'gray.400',
    display: 'flex',
    gap: 4,
    marginBottom: 3,
  })

  export const tab = style({
    typography: 'label.14',
    ':focus-visible': {
      outline: '2px solid',
      outlineColor: 'blue.700',
      outlineOffset: '2px',
    },
    ':hover': { color: 'foreground' },
    '&[aria-selected="true"]': {
      borderColor: 'foreground',
      color: 'foreground',
    },
    backgroundColor: 'transparent !custom',
    borderBottom: '2px solid',
    borderColor: 'transparent !custom',
    color: 'gray.900',
    cursor: 'pointer',
    marginBottom: '-1px !custom',
    paddingBlock: 2,
  })
}
