# Examples

Each example is a small project with an `uphill.yaml` and the scripts it runs. Uphill reads `uphill.toml` the same way. The scripts are TypeScript run by [Bun](https://bun.sh). They run the tools each project already uses and turn what those tools report into metric lines.

Every example misses a few targets on purpose, so its report shows how misses, opportunities, and contextual targets read.

| Example | Tools | Categories | Contextual targets |
| --- | --- | --- | --- |
| [`typescript-library`](typescript-library) | Bun's bundler and test runner, mitata, TypeScript, oxlint, oxfmt | Footprint, speed, and quality of a formatting library: entry and export sizes, tree-shaking, import and call times, tests, coverage, type errors, lint, and formatting | The root entry may cost the sum of its subpaths. Formatters that call `Intl` get more time per call. |
| [`web-app`](web-app) | Lighthouse | Performance, accessibility, best practices, and SEO of a marketing site: Lighthouse's metrics on each page, and each weighted audit as a check | The landing page loads a hero image, so it gets more time to paint and more bytes. |
| [`rust-api`](rust-api) | Cargo, clippy, rustfmt | Footprint, latency, reliability, and quality of an HTTP API on Rust's standard library | Each endpoint's latency target comes from the work it does, and its tail may take three times as long. |
| [`lighthouse`](lighthouse) | Lighthouse | Lighthouse 13.6.0 itself: every category, every audit that counts toward a score with Lighthouse's weight, and each page as a subject, so each page scores as its Lighthouse report does | Each metric takes Lighthouse's control points, mobile or desktop, as `target` and `poor`. |

```sh
# Install the example's tools, then score every category and write .uphill/report.md.
cd examples/web-app
bun install
uphill
```

A target comes from one of three places. `target` in `uphill.yaml` is the default for every item. A script can write its own target on a line, such as `latency /health 0.29ms target 500µs`, when it knows the context. `targets` in `uphill.yaml` sets targets by item pattern, such as `{ index.html: 8kB }`, and has the final say.

The web app and the Lighthouse example need Chrome. Lighthouse finds an installed Chrome, or the one at `CHROME_PATH`. Their performance categories write times, so Uphill runs them 5 times, and a full run takes a few minutes.

The Lighthouse example reproduces Lighthouse's scores. An audit with nothing to audit writes `skip`, as Lighthouse marks it not applicable. To compare the scores with Lighthouse's own reports, save the reports with `LIGHTHOUSE_REPORTS` and run its check:

```sh
# Run Lighthouse once per page, score each saved report, and compare every score.
cd examples/lighthouse
LIGHTHOUSE_REPORTS=.uphill/lighthouse uphill
LIGHTHOUSE_REPORTS=.uphill/lighthouse bun metrics/verify.ts
```

Uphill itself is measured the same way. Its [`.agents/uphill.yaml`](../.agents/uphill.yaml) tracks the release binary size, its dependencies, and its startup time.
