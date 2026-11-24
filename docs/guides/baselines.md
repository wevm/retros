# Baselines

A baseline is the last accepted value of a metric. Uphill compares every run with the baselines, and `--require-baseline` fails a run that differs from them.

## Overview

`uphill --update` writes each metric's accepted value to `uphill.baseline.yaml`, beside the configuration. Both files are committed, so accepting a regression or loosening a script's target is a reviewed change, like any other line of code.

Baselines hold sizes, counts, checks, and the targets scripts write on their lines. These are the same on every machine. Timings differ between machines, so a baseline never holds them. The [Comparing Revisions](./comparing.md) guide compares timings instead.

```yaml title="uphill.baseline.yaml"
# uphill --update wrote this file. The configuration keeps only what people write.
footprint:
  bundle-size: 6.9kB
```

## Walkthrough

This walkthrough uses the `pages` category from [Writing Scripts](./scripts.md), which writes `page-size` and `has-title` for each page in `dist/`.

### Record the Values

`uphill --update` runs every category and writes what it measured. A metric with items takes one line per item, so a later change to one item shows as one line in a diff. The configuration stays as it was.

```text
$ uphill --update
...
Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html, uphill.baseline.yaml
```

```yaml title="uphill.baseline.yaml"
# The pages category writes two metrics per page.
pages:
  page-size:
    about.html: 9015B
    index.html: 14034B
    pricing.html: 31037B
  has-title:
    about.html: fail
    index.html: pass
    pricing.html: pass
```

### Read the Changes

After a change adds a blog page, grows the pricing page, and gives the about page a title, `uphill` lists each item that differs from its baseline. Each change is worse, better, new, or gone.

```text
$ uphill
...
Since the baseline
  worse   pages  page-size pricing.html  31 kB → 33 kB
  new     pages  page-size blog.html     4 kB
  new     pages  has-title blog.html     pass
  better  pages  has-title about.html    fail → pass
```

### Gate the Change

`--require-baseline` exits with status 1 when anything differs from the baselines, better or worse, or when no metric has a baseline. The baselines on the main branch then always match its code, and each improvement becomes the new floor.

```text
$ uphill --require-baseline
...
4 changes since the baseline. Run `uphill --update` to accept them
```

### Accept the Change

`uphill --update` accepts the run. The diff of the baseline file shows each accepted change on its own line, and the next run with `--require-baseline` passes.

```diff title="uphill.baseline.yaml"
   page-size:
     about.html: 9015B
+    blog.html: 4034B
     index.html: 14034B
-    pricing.html: 31037B
+    pricing.html: 33037B
   has-title:
-    about.html: fail
+    about.html: pass
+    blog.html: pass
     index.html: pass
     pricing.html: pass
```

## Recipes

### Record Line Targets

A baseline records the target a script writes on a line, after the value. When a script changes its own target, the run reports it as looser or tighter, so a loosened target cannot pass review unseen.

```yaml title="uphill.baseline.yaml"
# The chains entry point wrote a 15 kB target on its line.
footprint:
  entry-size:
    chains: 12033B target 15kB
    index: 6112B
    utils: 1132B
```

```text
Since the baseline
  looser  footprint  entry-size chains  target 15 kB → 20 kB
```

### Varying Values

A size or count that differs between runs, such as peak memory, sets `varies: true`. Uphill takes its median across rounds like a time, and its baseline records only its line targets.

```yaml title="uphill.yaml"
# Peak memory differs between runs, so it is scored on the median of 5 rounds.
speed:
  run: sh metrics/speed.sh
  peak-memory: { target: 64MB, varies: true }
```

### Baselines in TOML

A project with `uphill.toml` gets `uphill.baseline.toml`. A metric with items takes a table of its own, one line per item.

```toml title="uphill.baseline.toml"
# uphill --update wrote this file beside uphill.toml.
[footprint.entry-size]
chains = "12033B target 15kB"
index = "6112B"
utils = "1132B"
```

### Move Older Baselines

Older configurations set a `baseline` key on each metric. Uphill still reads it, and `uphill --update` moves every one into the baseline file and removes the key, keeping the rest of the configuration as it was.

```diff title="uphill.yaml"
 footprint:
   run: sh metrics/footprint.sh
-  bundle-size: { target: 5kB, baseline: 6.9kB }
+  bundle-size: { target: 5kB }
```

### Trace a Metric

The baseline file lives in Git, so its history is the metric's history. `uphill log` lists the commits that changed a baseline, and `uphill blame` names the last one. With `--require-baseline` in CI, that commit is the change that moved the metric.

```text
$ uphill log footprint source-size
footprint source-size in the baselines of uphill.baseline.yaml

  fd268f9  2026-11-23  172 B  from 138 B  accept size
  84337c9  2026-11-23  138 B              init
```

### Gate in CI

A CI job that runs `uphill --require-baseline` fails each pull request that changes a size, count, check, or line target without running `uphill --update`. The [GitHub Actions](./github-actions.md) guide passes it through the `args` input.

```yaml title=".github/workflows/uphill.yml"
# Fail the pull request when a value differs from its baseline.
- uses: wevm/uphill@main
  with:
    args: --require-baseline
```

## More

- [Comparing Revisions](./comparing.md): Compare timings with another revision.
- [Targets](./targets.md): Set targets per item and by pattern.
- [Configuration](../reference/configuration.md): The baseline key and its format.
- [CLI](../reference/cli.md): --update, --require-baseline, and exit status.
