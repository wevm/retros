# Examples

Each example is a small project with a `retros.yaml` and the scripts it runs. The scripts are TypeScript run by [Bun](https://bun.sh). They run the tools each project already uses and turn what those tools report into metric lines.

Every example misses a few targets on purpose, so its report shows how misses, opportunities, and contextual targets read.

| Example | Tools | Categories | Contextual targets |
| --- | --- | --- | --- |
| [`typescript-library`](typescript-library) | Bun's bundler and test runner, mitata, TypeScript, oxlint, oxfmt | Footprint, speed, and quality of a formatting library: entry and export sizes, tree-shaking, import and call times, tests, coverage, type errors, lint, and formatting | The root entry may cost the sum of its subpaths. Formatters that call `Intl` get more time per call. |
| [`web-app`](web-app) | Lighthouse | Performance, accessibility, best practices, and SEO of a marketing site: Lighthouse's metrics on each page, and each weighted audit as a check | The landing page loads a hero image, so it gets more time to paint and more bytes. |
| [`rust-api`](rust-api) | Cargo, clippy, rustfmt | Footprint, latency, reliability, and quality of an HTTP API on Rust's standard library | Each endpoint's latency target comes from the work it does, and its tail may take three times as long. |

```sh
# Install the example's tools, then score every category and write .retros/report.md.
cd examples/web-app
bun install
retros
```

A target comes from one of three places. `target` in `retros.yaml` is the default for every item. A script can write its own target on a line, such as `latency /health 0.29ms target 500µs`, when it knows the context. `targets` in `retros.yaml` sets targets by item pattern, such as `{ index.html: 8kB }`, and has the final say.

The web app needs Chrome. Lighthouse finds an installed Chrome, or the one at `CHROME_PATH`. Its performance category writes times, so Retros runs it 5 times, and a full run takes about 3 minutes.

Retros itself is measured the same way. Its [`.agents/retros.yaml`](../.agents/retros.yaml) tracks the release binary size, its dependencies, and its startup time.
