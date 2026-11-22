# Examples

Each example is a small project with a `retros.toml` and the scripts it runs. The scripts are TypeScript run by [Bun](https://bun.sh), with no packages to install. The Rust example also needs Cargo.

Every example misses a few targets on purpose, so its report shows how misses, opportunities, and contextual targets read.

| Example | Categories | Contextual targets |
| --- | --- | --- |
| [`typescript-library`](typescript-library) | Footprint, speed, and quality of a formatting library: entry and export sizes, tree-shaking, import and call times, tests, coverage, and TSDoc | The root entry may cost the sum of its subpaths. Formatters that call `Intl` get more time per call. |
| [`web-app`](web-app) | Performance, accessibility, best practices, and SEO of a marketing site, like Lighthouse | The landing page may weigh more than the others. Each image's target comes from the screen area it covers. |
| [`rust-api`](rust-api) | Footprint, latency, and reliability of an HTTP API on Rust's standard library | Each endpoint's latency target comes from the work it does, and its tail may take three times as long. |

```sh
# Score every category, list the opportunities, and write .retros/report.md.
cd examples/web-app
retros
```

A target comes from one of three places. `target` in `retros.toml` is the default for every item. A script can write its own target on a line, such as `latency /health 0.29ms target 500µs`, when it knows the context. `targets` in `retros.toml` sets targets by item pattern, such as `{ "index.html" = "8kB" }`, and has the final say.

Retros itself is measured the same way. Its [`retros.toml`](../retros.toml) tracks the release binary size, its dependencies, and its startup time.
