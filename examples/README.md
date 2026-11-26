# Examples

Each example is a small project with an `uphill.yaml` and the scripts it runs. Uphill reads `uphill.toml` the same way. The scripts are TypeScript run by [Bun](https://bun.sh). They run the tools each project already uses and turn what those tools report into metric lines.

Every example misses a few targets on purpose, so its report shows how misses, opportunities, and contextual targets read.

| Example | Tools | Categories | Contextual targets |
| --- | --- | --- | --- |
| [`typescript-library`](typescript-library) | Bun's bundler and test runner, mitata, and the typescript pack | Footprint, speed, testing, and correctness of a formatting library: entry and export sizes, tree-shaking, import and call times, tests, and coverage, beside the typescript pack's categories | The root entry may cost the sum of its subpaths. Formatters that call `Intl` get more time per call. |
| [`web-app`](web-app) | Lighthouse and the web pack | Performance and SEO of a marketing site, with Lighthouse's metrics on each page, and the web pack's accessibility and best practices | The landing page loads a hero image, so it gets more time to paint and more bytes. |
| [`rust-api`](rust-api) | Cargo and the rust pack | Footprint, latency, reliability, and testing of an HTTP API on Rust's standard library, beside the rust pack's categories | Each endpoint's latency target comes from the work it does, and its tail may take three times as long. |

```sh
# Install the example's tools, then score every category and write .uphill/report.md.
cd examples/web-app
bun install
uphill
```

Copy the categories and scripts that fit into another project, and read the example's `README.md`, which says what to fit. The packs in [`packs/`](../packs) measure common conventions without a copy.

Each example keeps its targets in `uphill.baseline.yaml`. A record with only a target, such as `binary-size: { target: 1MB }`, is the default for every item. A script can write its own target on a line, such as `latency /health 0.29ms target 500µs`, when it knows the context. A pattern such as `"viem/chains/*": { target: 1kB }`, or an item's own target such as `index.html: { target: 8kB }`, overrides the line. `uphill --update` writes each item's accepted value as its `baseline`, beside the `target` it scored against.

The web app needs Chrome. Lighthouse finds an installed Chrome, or the one at `CHROME_PATH`. Its performance category writes times, so Uphill runs it 5 times, and a full run takes about 3 minutes.

Uphill itself is measured the same way. Its [`.agents/uphill/uphill.yaml`](../.agents/uphill/uphill.yaml) tracks the release binary size, its dependencies, and its startup time.
