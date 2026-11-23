/** Introduces Uphill with its loop: configure, measure, and score. @module */
import { createFileRoute } from '@tanstack/react-router'
import { Link } from '../components/Link.js'
import { Shell } from '../components/Shell.js'
import * as Head from '../Head.js'
import { style } from '../zyzz.config.js'

export const Route = createFileRoute('/')({
  component: Home,
  head: () =>
    Head.page({
      description: Head.description,
      title: 'Uphill · Continuous Metrics and Hill Climbing',
    }),
})

// The same example as the README: a category, a script that writes one metric line, and the score.
const steps = [
  {
    code: `footprint:
  run: sh .agents/metrics/footprint.sh
  bundle-size: { target: 5kB }`,
    label: '.agents/uphill.yaml',
    title: 'Set a target',
  },
  {
    code: `echo "bundle-size $(gzip -c dist/index.js | wc -c)B" \\
  >> "$UPHILL_METRICS"`,
    label: '.agents/metrics/footprint.sh',
    title: 'Measure with a script',
  },
  {
    code: `$ uphill
■  75  footprint
  ■  75  bundle-size  6.9 kB  target 5 kB

Opportunities
  +15  footprint  bundle-size  6.9 kB → 5 kB`,
    label: 'Terminal',
    title: 'Score and climb',
  },
] as const

const features = [
  {
    text: 'Scripts run the tools a project already uses, in any language, and write metric lines to a file.',
    title: 'Any Project',
  },
  {
    text: 'A value at its target scores 90, twice the target scores 50, and four times the target scores 10.',
    title: 'Scores',
  },
  {
    text: 'Opportunities rank the changes that raise a category most, so the next step is always named.',
    title: 'Next Steps',
  },
  {
    text: 'Baselines record accepted sizes, counts, and checks in the configuration, so every change shows in review.',
    title: 'Baselines',
  },
  {
    text: 'Comparisons run timings in interleaved pairs against another revision, and report only the changes the pairs agree on.',
    title: 'Honest Timings',
  },
  {
    text: 'Reports go to JSON, Markdown, and one self-contained HTML file. The GitHub Action posts the Markdown on pull requests.',
    title: 'Reports',
  },
] as const

function Home() {
  return (
    <Shell>
      <section {...styles.hero()}>
        <h1 {...styles.title()}>Continuous metrics and hill climbing</h1>
        <p {...styles.lede()}>
          Uphill scores a project against its own targets. A metric is anything
          a script can measure: bundle size, page load time, request latency,
          test coverage, or lint warnings. Uphill scores each one from 0 to 100
          and names the fixes worth the most points.
        </p>
        <div {...styles.actions()}>
          <Link href="/docs/introduction/getting-started" {...styles.primary()}>
            Get Started
          </Link>
          <a
            href="https://github.com/wevm/uphill"
            rel="noreferrer"
            target="_blank"
            {...styles.secondary()}
          >
            GitHub
          </a>
        </div>
        <pre {...styles.install()}>
          <code>
            cargo install --locked --git https://github.com/wevm/uphill uphill
          </code>
        </pre>
      </section>

      <section aria-label="How it works" {...styles.steps()}>
        {steps.map((step, index) => (
          <div key={step.title} {...styles.step()}>
            <h2 {...styles.stepTitle()}>
              <span {...styles.number()}>{index + 1}</span>
              {step.title}
            </h2>
            <div {...styles.panel()}>
              <div {...styles.panelLabel()}>{step.label}</div>
              <pre {...styles.panelCode()}>
                <code>{step.code}</code>
              </pre>
            </div>
          </div>
        ))}
      </section>

      <section aria-label="Features" {...styles.features()}>
        {features.map((feature) => (
          <div key={feature.title}>
            <h2 {...styles.featureTitle()}>{feature.title}</h2>
            <p {...styles.featureText()}>{feature.text}</p>
          </div>
        ))}
      </section>
    </Shell>
  )
}

namespace styles {
  export const hero = style({
    marginInline: 'auto !custom',
    maxWidth: '48rem !custom',
    paddingBottom: 12,
    paddingInline: 6,
    paddingTop: 24,
    textAlign: 'center',
    '@media (max-width: 640px)': { paddingInline: 4, paddingTop: 16 },
  })

  export const title = style({
    typography: 'heading.56',
    '@media (max-width: 640px)': { typography: 'heading.40' },
  })

  export const lede = style({
    typography: 'copy.18',
    color: 'gray.900',
    marginTop: 6,
  })

  export const actions = style({
    display: 'flex',
    gap: 3,
    justifyContent: 'center',
    marginTop: 8,
  })

  const button = {
    typography: 'button.14',
    ':focus-visible': {
      outline: '2px solid',
      outlineColor: 'blue.700',
      outlineOffset: '2px',
    },
    alignItems: 'center',
    borderRadius: 'md',
    display: 'inline-flex',
    height: 10,
    paddingInline: 4,
    textDecoration: 'none',
  } as const

  export const primary = style({
    ...button,
    ':hover': { backgroundColor: 'gray.900' },
    backgroundColor: 'foreground',
    color: 'background.surface',
  })

  export const secondary = style({
    ...button,
    ':hover': { backgroundColor: 'gray.100' },
    border: '1px solid',
    borderColor: 'gray.400',
    color: 'foreground',
  })

  export const install = style({
    typography: 'copy.13.mono',
    backgroundColor: 'background.primary',
    border: '1px solid',
    borderColor: 'gray.400',
    borderRadius: 'lg',
    marginTop: 8,
    overflowX: 'auto',
    paddingBlock: 3,
    paddingInline: 4,
    textAlign: 'left',
  })

  export const steps = style({
    display: 'grid',
    gap: 6,
    gridTemplateColumns: 'repeat(3, minmax(0, 1fr)) !custom',
    marginInline: 'auto !custom',
    maxWidth: '80rem !custom',
    paddingBlock: 12,
    paddingInline: 6,
    '@media (max-width: 960px)': {
      gridTemplateColumns: 'minmax(0, 1fr) !custom',
    },
    '@media (max-width: 640px)': { paddingInline: 4 },
  })

  export const step = style({
    display: 'flex',
    flexDirection: 'column',
    gap: 3,
  })

  export const stepTitle = style({
    typography: 'heading.16',
    alignItems: 'center',
    display: 'flex',
    gap: 2,
  })

  export const number = style({
    typography: 'label.12',
    alignItems: 'center',
    border: '1px solid',
    borderColor: 'gray.500',
    borderRadius: '9999px !custom',
    display: 'inline-flex',
    height: 6,
    justifyContent: 'center',
    width: 6,
  })

  export const panel = style({
    border: '1px solid',
    borderColor: 'gray.400',
    borderRadius: 'lg',
    display: 'flex',
    flex: '1 !custom',
    flexDirection: 'column',
    overflow: 'hidden',
  })

  export const panelLabel = style({
    typography: 'label.12',
    backgroundColor: 'gray.100',
    borderBottom: '1px solid',
    borderColor: 'gray.400',
    color: 'gray.900',
    fontFamily: 'mono',
    paddingBlock: 1.5,
    paddingInline: 4,
  })

  export const panelCode = style({
    typography: 'copy.13.mono',
    backgroundColor: 'background.primary',
    flex: '1 !custom',
    margin: '0 !custom',
    overflowX: 'auto',
    padding: 4,
  })

  export const features = style({
    display: 'grid',
    gap: 8,
    gridTemplateColumns: 'repeat(3, minmax(0, 1fr)) !custom',
    marginInline: 'auto !custom',
    maxWidth: '80rem !custom',
    paddingBottom: 24,
    paddingInline: 6,
    paddingTop: 12,
    '@media (max-width: 960px)': {
      gridTemplateColumns: 'repeat(2, minmax(0, 1fr)) !custom',
    },
    '@media (max-width: 640px)': {
      gridTemplateColumns: 'minmax(0, 1fr) !custom',
      paddingInline: 4,
    },
  })

  export const featureTitle = style({ typography: 'heading.16' })

  export const featureText = style({
    typography: 'copy.14',
    color: 'gray.900',
    marginTop: 2,
  })
}
