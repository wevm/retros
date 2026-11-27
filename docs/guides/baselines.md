# Baselines

A baseline is the last accepted value of a metric. Uphill compares every run with the baselines, and `--require-baseline` fails a run that differs from them.

## Overview

`uphill --update` writes each metric's accepted value to `uphill.baseline.yaml`, beside the configuration, next to its target. The project commits both files, so accepting a regression or loosening a target is a reviewed change, like any other line of code.

Baselines hold sizes, counts, and checks. These are the same on every machine. Timings differ between machines, so a baseline never holds them, and a timing keeps only its target. The [Comparing Revisions](./comparing.md) guide compares timings instead.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# The accepted value and the target, side by side.
footprint:
  bundle-size: { baseline: 6.9kB, target: 5kB }
```

## Walkthrough

This walkthrough uses the `pages` category from [Writing Scripts](./scripts.md), which writes `page-size` and `has-title` for each page in `dist/`.

### Record the Values

`uphill --update` runs every category and writes what it measured, beside each item's own target. A metric with items takes one line per item, so a later change to one item shows as one line in a diff. The configuration does not change.

```text
$ uphill --update
...
Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html, uphill.baseline.yaml
```

```yaml title=".agents/uphill/uphill.baseline.yaml"
# The pages category writes two metrics per page. Each page size scores against 20 kB, and
# every page but one has a title.
pages:
  page-size:
    "*": { target: 20kB }
    about.html: { baseline: 9015B }
    index.html: { baseline: 14034B }
    pricing.html: { baseline: 31037B }
  has-title:
    "*": { baseline: pass }
    about.html: { baseline: fail }
```

The baseline file leaves out an item's target when the pattern or default already sets it. A list of checks starts with a default record, `*`, that holds its most common check, and lists only the pages that differ.

### Read the Changes

After a change adds a blog page, grows the pricing page, and titles the about page, `uphill` lists each item that differs from its baseline. Each change is worse, better, new, or gone. The blog page has a title, which matches the default record, so only its size is new.

```text
$ uphill
...
Since the baseline
  worse   pages  page-size pricing.html  31 kB → 33 kB
  new     pages  page-size blog.html     4 kB
  better  pages  has-title about.html    fail → pass
```

### Gate the Change

`--require-baseline` exits with status 1 when anything differs from the baselines, better or worse, or when no metric has a baseline. The baselines on the main branch then always match its code, and each improvement becomes the new floor.

```text
$ uphill --require-baseline
...
3 changes since the baseline. Run `uphill --update` to accept them
```

### Accept the Change

`uphill --update` accepts the run. The diff of the baseline file shows each accepted change on its own line, and the next run with `--require-baseline` passes.

```diff title=".agents/uphill/uphill.baseline.yaml"
   page-size:
     "*": { target: 20kB }
     about.html: { baseline: 9015B }
+    blog.html: { baseline: 4034B }
     index.html: { baseline: 14034B }
-    pricing.html: { baseline: 31037B }
+    pricing.html: { baseline: 33037B }
   has-title:
     "*": { baseline: pass }
-    about.html: { baseline: fail }
```

## Recipes

### Record Line Targets

An item without its own target takes the target its script writes on the line, and `uphill --update` records it. Until then, the run reports it as looser or tighter than the pattern or default it replaces. Afterwards, the file's target has the final say.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# The chains entry point wrote a 15 kB target on its line, and the update recorded it.
footprint:
  entry-size:
    "*": { target: 5kB }
    chains: { baseline: 12033B, target: 15kB }
    index: { baseline: 6112B }
```

```text
Since the baseline
  looser  footprint  entry-size chains  target 5 kB → 15 kB
```

### Varying Values

A size or count that differs between runs, such as peak memory, sets `varies: true`. Uphill takes its median across rounds like a time, and its baseline records only its target.

A check that varies records nothing. A check of each recent commit is one: each new commit adds an item, so a baseline of its items would differ after every commit.

```yaml title=".agents/uphill/uphill.yaml"
# Peak memory is scored on the median of 5 rounds. The commit checks score each run, and the
# baseline file records none of their items.
speed:
  run: sh .agents/uphill/metrics/speed.sh
  peak-memory: { varies: true }
commits:
  run: sh .agents/uphill/metrics/commits.sh
  commit-type: { varies: true }
```

### Lists of Checks

A list of checks records its most common check as the default record, `*`, and only the items that differ from it. A tie goes to `pass`, then `skip`, then `fail`. A list where most items skip lists its passes and failures.

Files from earlier versions list the check of every item. Uphill still reads them, and the next `uphill --update` rewrites each list with its default record.

```diff title=".agents/uphill/uphill.baseline.yaml"
 # The update replaces the passing items with the default record.
 style:
   lint:
+    "*": { baseline: pass }
-    src/index.ts: { baseline: pass }
     src/legacy.ts: { baseline: fail }
-    src/util.ts: { baseline: pass }
```

### Baselines in TOML

A project with `uphill.toml` uses `uphill.baseline.toml`. A metric with items takes a table of its own, one line per item.

```toml title=".agents/uphill/uphill.baseline.toml"
# The baseline file beside uphill.toml.
[footprint.entry-size]
"*" = { target = "5kB" }
chains = { baseline = "12033B", target = "15kB" }
index = { baseline = "6112B" }
```

### Move Older Keys

Older configurations set `target`, `targets`, and `baseline` keys on each metric. Uphill still reads them, and `uphill --update` moves every one into the baseline file and removes the key. The rest of the configuration does not change.

```diff title=".agents/uphill/uphill.yaml"
 footprint:
   run: sh .agents/uphill/metrics/footprint.sh
-  bundle-size: { target: 5kB, baseline: 6.9kB }
+  bundle-size: {}
```

```yaml title=".agents/uphill/uphill.baseline.yaml"
# The update writes the moved target beside the new value.
footprint:
  bundle-size: { baseline: 7.1kB, target: 5kB }
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

A CI job that runs `uphill --require-baseline` fails each pull request that changes a size, count, or check without running `uphill --update`. The [GitHub Actions](./github-actions.md) guide passes it through the `args` input.

```yaml title=".github/workflows/uphill.yml"
# Fail the pull request when a value differs from its baseline.
- uses: wevm/uphill@main
  with:
    args: --require-baseline
```

## More

- [Comparing Revisions](./comparing.md): Compare timings with another revision.
- [Targets](./targets.md): Set targets per item and by pattern.
- [Configuration](../reference/configuration.md): The baseline file and its format.
- [CLI](../reference/cli.md): --update, --require-baseline, and exit status.
