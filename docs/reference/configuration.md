# Configuration

`uphill.yaml` or `uphill.toml` declares the categories, the command that measures each one, and how each metric scores. Both formats accept the same keys. The [baseline file](#baseline-file) beside it holds each metric's targets and accepted values.

## Files

Uphill reads `uphill.yaml` or `uphill.toml` in `.agents/uphill/`, in `.agents/`, or at the project root, from the current directory or the nearest parent. One project with more than one of these files is an error. The project root is the directory that holds the file or `.agents/`.

`.agents/uphill/` is the default. It keeps Uphill's files in one directory: the configuration, its baseline file, the scripts, and the [rules file](cli.md#the-rules-file).

Scripts run from the project root, and reports go to `.uphill/` in it. Each top-level key is a category, apart from `run`, `setup`, and `timeout`.

#### YAML

```yaml title=".agents/uphill/uphill.yaml"
# Two categories, each measured by its own script. The targets are in uphill.baseline.yaml.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  entry-size: {}

speed:
  run: bun .agents/uphill/metrics/speed.ts
  import-time: {}
```

#### TOML

```toml title=".agents/uphill/uphill.toml"
# Two categories, each measured by its own script. The targets are in uphill.baseline.toml.
[footprint]
run = "sh .agents/uphill/metrics/footprint.sh"
entry-size = {}

[speed]
run = "bun .agents/uphill/metrics/speed.ts"
import-time = {}
```

## Names

A category or metric name starts with an ASCII letter or digit, and continues with ASCII letters, digits, `-`, `_`, and `.`. `report.md` and `report.html` show a name in words unless a `title` is set, so `request-out` reads "Request out".

```yaml title=".agents/uphill/uphill.yaml"
# Valid names: footprint, entry-size, p95_latency, and v2.startup.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  entry-size: {}
```

## Top-Level Keys

### `allowed-packs`

- **Type:** list of patterns
- **Default:** none, which allows every source

Patterns that each remote pack in `packs` must match, where `*` matches any characters. Every run, `uphill add`, and `uphill update` reject a pack that no pattern matches.

```yaml title=".agents/uphill/uphill.yaml"
# Only packs from the wevm organization.
allowed-packs: [wevm/*]
```

### `packs`

- **Type:** list of sources, or mappings with `pack`, `categories`, and `install`
- **Default:** none

The packs whose categories run after the project's own. Categories with the same name merge into one, and a project category merges with a pack's category of its name (see [Packs](../guides/packs.md#merge-categories)). A remote entry is pinned to a full commit, and a run fetches its files into the cache of packs. A comment at the end of the line names the ref that `uphill update` follows. `install: true` approves installing the pack's dependencies from its lockfile, without install scripts.

```yaml title=".agents/uphill/uphill.yaml"
packs:
  # A remote pack, pinned to a commit. `uphill update` follows main.
  - wevm/uphill/packs/rust@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce # main
  # Only two categories, with its dependencies installed.
  - { pack: wevm/uphill/packs/typescript@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce, categories: [style, testing], install: true } # main
  # A local pack, which the project's own commit pins.
  - ./packs/api
```

### `run`

- **Type:** string
- **Default:** none

The command for every category that sets no `run`. Categories with the same command share one run of it, and each line goes to the category that lists its metric. One metric in two categories that share a run is an error.

```yaml title=".agents/uphill/uphill.yaml"
# Both categories read the lines of one run of bench.sh.
run: sh bench.sh

footprint:
  binary-size: {}

speed:
  startup: {}
```

### `setup`

- **Type:** string
- **Default:** none

A command that prepares each checkout that `--compare` and `uphill blame` create, such as a package install. It runs once from the root of the new checkout, before any script, with the top-level `timeout`. A failed setup stops the run with exit status 2.

```yaml title=".agents/uphill/uphill.yaml"
# Install packages in each worktree that Uphill creates.
setup: bun install --frozen-lockfile
```

### `timeout`

- **Type:** duration, such as `500ms`, `30s`, `10m`, or `1h`
- **Default:** `10m`

The time each script may run in a category that sets no `timeout`, and the time `setup` may run. When the time passes, Uphill stops the script and every process it started.

```yaml title=".agents/uphill/uphill.yaml"
# Give every script 2 minutes.
timeout: 2m
```

## Category Keys

### `run`

- **Type:** string
- **Default:** the top-level `run`

The command that measures the category. Uphill runs it through `sh -c` from the project root, with `$UPHILL_METRICS` set to a new file for the script's metric lines. A category without a `run` and without a top-level `run` is an error.

```yaml title=".agents/uphill/uphill.yaml"
# Measure the footprint category with a shell script.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  bundle-size: {}
```

### `timeout`

- **Type:** duration
- **Default:** the top-level `timeout`

The time the category's script may run.

```yaml title=".agents/uphill/uphill.yaml"
# Give the end-to-end suite 30 minutes.
e2e:
  run: sh .agents/uphill/metrics/e2e.sh
  timeout: 30m
  passed: {}
```

### `title`

- **Type:** string
- **Default:** the name in words

The name that `report.md`, `report.html`, and `report.json` show. The terminal and baselines keep the name.

```yaml title=".agents/uphill/uphill.yaml"
# Show "Bundle footprint" in reports.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  title: Bundle footprint
  bundle-size: {}
```

### `description`

- **Type:** string
- **Default:** none

What the category measures, which `report.html` and `report.json` show.

```yaml title=".agents/uphill/uphill.yaml"
# Describe the category for readers of the report.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  description: Gzipped size of the published entry points.
  bundle-size: {}
```

## Metric Keys

Every other key of a category is a metric. A metric is a mapping of the keys below. A metric without keys, such as `modules: {}`, has a score only when the baseline file sets its target. A category must declare at least one metric.

```yaml title=".agents/uphill/uphill.yaml"
# Two metrics. The baseline file decides which ones score.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  bundle-size: {}
  modules: {}
```

### `poor`

- **Type:** a size, a time, or a number
- **Default:** twice the target, or half the target when higher is better

The value that scores 50, Lighthouse's median control point. It needs a default target in the baseline file, in the same kind of unit, and it is worse than the target. An item with its own target scales `poor` by its target over the default.

```yaml title=".agents/uphill/uphill.yaml"
# Lighthouse's mobile Largest Contentful Paint: 50 at 4 s, with a 2.5 s target in the baseline file.
largest-contentful-paint: { poor: 4000ms }
```

### `better`

- **Type:** `lower` or `higher`
- **Default:** `lower`

The direction that improves the metric. With `higher`, half the target scores 50, and a list collapses to its p10 by default.

```yaml title=".agents/uphill/uphill.yaml"
# More coverage is better.
coverage: { better: higher }
```

### `score`

- **Type:** `p90`, `p10`, `median`, `max`, `min`, or `sum`
- **Default:** `p90`, or `p10` when `better` is `higher`

How a list collapses to one score. Items rank by value against their own target, and a percentile takes the item at its nearest rank. A `sum` scores the total of the values against the total of the targets.

```yaml title=".agents/uphill/uphill.yaml"
# Score the total size of every chunk.
chunk-size: { score: sum }
```

### `weight`

- **Type:** a number, 0 or greater
- **Default:** `1`

The metric's weight in the category score, which is the weighted mean of the metric scores. A metric with weight 0 still has a score, but the category mean leaves it out.

```yaml title=".agents/uphill/uphill.yaml"
# Count throughput twice.
throughput: { better: higher, weight: 2 }
```

### `varies`

- **Type:** `true` or `false`
- **Default:** `false`

Marks a size, count, or check that differs between runs, such as peak memory. Uphill repeats the script of a size or count and takes the median like a time. The baseline records only targets, so a check that varies records nothing.

```yaml title=".agents/uphill/uphill.yaml"
# Peak memory differs between runs, and each new commit adds a commit check.
peak-memory: { varies: true }
commit-type: { varies: true }
```

### `title`

- **Type:** string
- **Default:** the name in words

The name that `report.md`, `report.html`, and `report.json` show.

```yaml title=".agents/uphill/uphill.yaml"
# Show "Time to first request" instead of "Request out".
request-out: { title: Time to first request }
```

### `description`

- **Type:** string
- **Default:** none

The metric's rule, written as an instruction. [`uphill rules`](cli.md#uphill-rules) prints it for agents, and `report.html` and `report.json` show it.

```yaml title=".agents/uphill/uphill.yaml"
# State the rule that keeps the metric on target.
page-size: { description: Keep the HTML of each page small. Inline no images. }
```

## Baseline File

`uphill.baseline.yaml` or `uphill.baseline.toml` sits beside the configuration, in the same format. It holds each metric's targets and the values `uphill --update` accepted, side by side. Commit both files.

Categories and metrics keep the order the configuration declares. A metric with items takes one line per item, and a list of checks lists only the items that differ from its [default record](#default-record). A metric with subjects takes one entry per subject. `uphill --update` rewrites the whole file, so comments do not survive.

#### YAML

```yaml title=".agents/uphill/uphill.baseline.yaml"
# The targets, and the accepted values beside them, of the configuration in uphill.yaml.
footprint:
  bundle-size: { baseline: 6.9kB, target: 5kB }
  entry-size:
    "*": { target: 5kB }
    chains: { baseline: 12033B, target: 15kB }
    index: { baseline: 6112B }
  export-size:
    node:
      format: { baseline: 900B, target: 1kB }
      parse: { baseline: 1.1kB, target: 1kB }
  links:
    "*": { baseline: pass }
    about.html: { baseline: fail }
```

#### TOML

```toml title=".agents/uphill/uphill.baseline.toml"
# The same file beside uphill.toml. A metric with items takes a table of its own.
[footprint]
bundle-size = { baseline = "6.9kB", target = "5kB" }

[footprint.entry-size]
"*" = { target = "5kB" }
chains = { baseline = "12033B", target = "15kB" }
index = { baseline = "6112B" }
```

### Records

- **Type:** a mapping with `baseline`, `target`, or both, such as `{ baseline: 6.9kB, target: 5kB }`

A record holds one measurement. `baseline` is the size, count, or check `uphill --update` accepted, and `target` is the value that scores 90. A timing or a metric that `varies` records only its target. A record with only a target sets the target of a metric that has no accepted value yet. Uphill still reads the short form of older files, such as `6.9kB target 5kB`.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# A size with its target, an unscored count, and a timing's target.
footprint:
  bundle-size: { baseline: 6.9kB, target: 5kB }
  modules: { baseline: 412 }
speed:
  startup: { target: 20ms }
```

### Targets

- **Type:** a size, a time, or a number, under `target`
- **Default:** none

The value that scores 90. It is greater than zero and in the same kind of unit as the metric's values. A list of checks scores the share of items that pass, so a check has no target.

An item's own target has the final say, then the first matching pattern, then the target on the item's line, then the default. `uphill --update` writes the target each item scored against as its own, unless a pattern or the default already sets it.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# Each page scores 90 at 20 kB and 50 at 40 kB, and the pricing page has its own target.
pages:
  page-size:
    "*": { target: 20kB }
    pricing.html: { baseline: 31037B, target: 40kB }
```

### Patterns

- **Type:** a key with `*` and a record with only a `target`
- **Default:** none

Targets by item for each item without its own target. In a pattern, `*` matches any run of characters, and the first matching pattern applies. The pattern `*` alone is the default, which a line's target replaces. A record with only a target, such as `bundle-size: { target: 5kB }`, sets the default of a metric with items too.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# Each chain entry point scores against 1 kB, and every other entry point against 5 kB.
footprint:
  export-size:
    "*": { target: 5kB }
    "viem/chains/*": { target: 1kB }
```

### Default Record

- **Type:** the key `*` and a record with a `baseline`, such as `"*": { baseline: pass }`
- **Default:** the most common check of the list

The check of each item without its own record, in a list of checks. `uphill --update` writes the most common check, with a tie going to `pass`, then `skip`, then `fail`. It then lists only the items that differ.

A new item that matches the default is not new. A new item that differs compares with the default, so a new failing item is worse. An item that the default covers and that is gone reports nothing. Files that list every item still read.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# Every file passes the lint, except one that fails and one that skips.
style:
  lint:
    "*": { baseline: pass }
    src/legacy.ts: { baseline: fail }
    src/generated.ts: { baseline: skip }
```

## Older Keys

Older configurations set targets and baselines on each metric. Uphill still reads these keys, so older revisions compare and keep their history. `uphill --update` moves each one into the baseline file and removes it from the configuration, keeping its comments and formatting. When both set a metric, the baseline file applies.

- **`target`:** The default target, which becomes the `*` pattern.
- **`targets`:** Item patterns and their targets, which become patterns.
- **`baseline`:** The last accepted value, or a mapping of items to values, which becomes the records.

```diff title=".agents/uphill/uphill.yaml"
 # uphill --update moves the target and the baseline into uphill.baseline.yaml.
-bundle-size: { target: 5kB, baseline: 6.9kB }
+bundle-size: {}
```
