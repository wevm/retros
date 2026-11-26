# Writing Scripts

A script measures a project and writes one metric line per measurement. Uphill runs the script, reads the lines, and scores them.

## Overview

A script runs the tools a project already uses, such as its bundler, test runner, or linter, and turns their output into metric lines. It appends each line to the file at `$UPHILL_METRICS`. Uphill creates a new file for each run.

Scripts can use any language. Uphill runs each `run` command through `sh -c` from the project root, the directory that holds `uphill.yaml` or `.agents/`. Standard output and standard error are logs.

#### Shell

```yaml title=".agents/uphill/uphill.yaml"
# Run the script with sh.
pages:
  run: sh .agents/uphill/metrics/pages.sh
  page-size: {}
```

```sh title=".agents/uphill/metrics/pages.sh"
# Write the size of each built page as one item of page-size.
for page in dist/*; do
  echo "page-size ${page#dist/} $(wc -c < "$page")B" >> "$UPHILL_METRICS"
done
```

#### TypeScript

```yaml title=".agents/uphill/uphill.yaml"
# Run the script with Bun, which runs TypeScript without a build step.
pages:
  run: bun .agents/uphill/metrics/pages.ts
  page-size: {}
```

```ts title=".agents/uphill/metrics/pages.ts"
// Write the size of each built page as one item of page-size.
import * as fs from 'node:fs'

for (const name of fs.readdirSync('dist')) {
  const size = fs.statSync(`dist/${name}`).size
  fs.appendFileSync(process.env.UPHILL_METRICS!, `page-size ${name} ${size}B\n`)
}
```

#### Rust

```yaml title=".agents/uphill/uphill.yaml"
# Run the script as a Cargo example.
pages:
  run: cargo run --quiet --example pages
  page-size: {}
```

```rust title="examples/pages.rs"
// Write the size of each built page as one item of page-size.
use std::{env, fs, io::Write};

fn main() -> std::io::Result<()> {
    let path = env::var("UPHILL_METRICS").expect("uphill sets UPHILL_METRICS");
    let mut metrics = fs::OpenOptions::new().append(true).open(path)?;

    for entry in fs::read_dir("dist")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        writeln!(metrics, "page-size {name} {}B", entry.metadata()?.len())?;
    }

    Ok(())
}
```

## Walkthrough

This walkthrough measures a static site in `dist/`. One script writes the size of each page, and a check for whether each page has a `<title>` element.

### Declare the Metrics

The `pages` category lists both metrics. The baseline file gives `page-size` a target, so each page scores against 20 kB. `has-title` writes checks, which need no target: a list of checks scores the share of items that pass.

```yaml title=".agents/uphill/uphill.yaml"
# The pages category runs one script, which writes two metrics per page.
pages:
  run: sh .agents/uphill/metrics/pages.sh
  page-size: {}
  has-title: {}
```

```yaml title=".agents/uphill/uphill.baseline.yaml"
# Each page scores against 20 kB.
pages:
  page-size: { target: 20kB }
```

### Write the Lines

Each line names the metric, then the item, then the value. A size takes a unit, and a check is `pass` or `fail`. A line can end with a note that says why the value is what it is.

```sh title=".agents/uphill/metrics/pages.sh"
# Write each page's size, and whether it has a <title>.
for page in dist/*.html; do
  name=${page#dist/}
  echo "page-size $name $(wc -c < "$page")B" >> "$UPHILL_METRICS"

  if grep -q '<title>' "$page"; then
    echo "has-title $name pass" >> "$UPHILL_METRICS"
  else
    echo "has-title $name fail note \"no <title> element\"" >> "$UPHILL_METRICS"
  fi
done
```

The script writes these lines for three pages:

```text
page-size about.html 9015B
has-title about.html fail note "no <title> element"
page-size index.html 14034B
has-title index.html pass
page-size pricing.html 31037B
has-title pricing.html pass
```

### Check the Script

`uphill check` runs each script once, without scoring. It fails on a line it cannot read, a metric the configuration lists but the script does not write, and a metric written with more than one kind of unit.

```text
$ uphill check
✓ pages  2 metrics

1 passed
```

### Score the Pages

`uphill` scores each item against its target and ranks the items worst first. A list collapses to its p90 item, so one large page decides the score of `page-size`. The note shows under the failed check.

```text
$ uphill
■  68  pages
  ■  68  page-size  p90 31 kB (pricing.html)  target 20 kB
                    31 kB  pricing.html
                    14 kB  index.html
                    9 kB   about.html
  ■  67  has-title  2 of 3 pass
                    fail  about.html
                          no <title> element

Opportunities
  +17  pages  has-title about.html    fail → pass
  +11  pages  page-size pricing.html  31 kB → 20 kB
```

## Recipes

### Quote an Item

An item or a note that contains a space or a tab takes double quotes. Inside quotes, a backslash escapes `"` and `\`.

```text
bench-time "ABI Decode (500 results)" 1.82ms
links "blog/first post.html" pass
```

### Measure Timings

When a script's first run writes a time, Uphill runs it 5 times and scores the median of each timing. Sizes, counts, and checks come from the first run, unless the metric sets `varies: true`. A script needs no repeat loop of its own.

```ts title=".agents/uphill/metrics/speed.ts"
// Time one import of the built entry point. Uphill repeats the script and takes the median.
import * as fs from 'node:fs'

const start = performance.now()
await import('../dist/index.js')
const time = performance.now() - start

fs.appendFileSync(process.env.UPHILL_METRICS!, `import-time ${time.toFixed(2)}ms\n`)
```

The category header names the rounds:

```text
■  87  speed  median of 5 rounds
  ■  87  import-time  5.3 ms  target 5 ms
```

### Use Subjects

A line that starts with a name and a colon belongs to that subject. Each subject scores on its own, which suits one script that measures several runtimes, packages, or builds. A category cannot mix lines with and without a subject.

```sh title="startup.sh"
# Write the startup time on each runtime as its own subject.
echo "node: startup 61ms" >> "$UPHILL_METRICS"
echo "bun: startup 18ms" >> "$UPHILL_METRICS"
```

```text
$ uphill
■  81  node: speed  median of 5 rounds
  ■  81  startup  61 ms  target 50 ms

● 100  bun: speed  median of 5 rounds
  ● 100  startup  18 ms  target 50 ms
```

### Share One Run

A top-level `run` applies to each category that sets none, and categories with the same `run` share one execution. Each line goes to the category that lists its metric.

```yaml title=".agents/uphill/uphill.yaml"
# Both categories read the lines of one run of bench.sh.
run: sh bench.sh

footprint:
  binary-size: {}

speed:
  startup: {}
```

### Fail a Run

A script that exits with a non-zero status gives its category no score, which is different from a score of 0. The report names the failure and keeps the end of standard error, and `uphill` exits with status 1.

```text
$ uphill
✗ quality  script exited with status 1
    src/index.ts: cannot find module './parse.js'
```

### Set a Timeout

A script may run for 10 minutes by default. The `timeout` key changes the limit for one category, or for every category at the top level. When the limit passes, Uphill stops the script and every process it started.

```yaml title=".agents/uphill/uphill.yaml"
# Give the end-to-end suite 30 minutes.
e2e:
  run: sh .agents/uphill/metrics/e2e.sh
  timeout: 30m
  passed: {}
```

## More

- [Targets](./targets.md): Set a target for each item and read opportunities.
- [Metric Lines](../reference/metric-lines.md): The full line format and its errors.
- [Units](../reference/units.md): Sizes, times, counts, and checks.
- [Configuration](../reference/configuration.md): Every key in uphill.yaml and uphill.toml.
