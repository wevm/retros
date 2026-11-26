# Targets

A target is a requirement the project must meet. Uphill scores each item against its own target, and opportunities name the changes that raise a score most.

## Overview

A value at its target scores 90, twice the target scores 50, and four times the target scores 10. The `poor` key moves the value that scores 50. Smaller values approach 100. Targets live in the baseline file beside the configuration, `uphill.baseline.yaml`.

Items can need different targets. A script that knows an item needs a larger or smaller target writes it on the line. A pattern in the baseline file overrides the script, and an item's own target in the file has the final say.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# Each entry point scores against 5 kB, except chains, which scores against 8 kB.
footprint:
  entry-size:
    "*": { target: 5kB }
    chains: { target: 8kB }
```

## Walkthrough

This walkthrough scores the gzipped size of three entry points: `index`, `utils`, and `chains`. The `chains` entry bundles every chain, so it is larger than the others by design.

### Set a Default

A record with only a target gives every item the same target. The list collapses to its p90 item, and `chains` is the worst item, so it decides the score.

```yaml title=".agents/uphill/uphill.yaml"
# The configuration names the metric and the script that writes it.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  entry-size: {}
```

```yaml title=".agents/uphill/uphill.baseline.yaml"
# Every entry point scores against 5 kB.
footprint:
  entry-size: { target: 5kB }
```

```sh title=".agents/uphill/metrics/footprint.sh"
# Write the gzipped size of each entry point.
for entry in index utils chains; do
  echo "entry-size $entry $(gzip -c dist/$entry.js | wc -c)B" >> "$UPHILL_METRICS"
done
```

```text
$ uphill
▲  36  footprint
  ▲  36  entry-size  p90 12 kB (chains)  target 5 kB
                     12 kB   chains
                     6.1 kB  index
                     1.1 kB  utils

Opportunities
  +45  footprint  entry-size chains  12 kB → 5 kB
```

### Target One Item

The script knows that `chains` holds every chain, so it writes a 15 kB target on that line. Items rank by value against their own target, so `index`, at 1.2 times its target, is now the worst item.

```sh title=".agents/uphill/metrics/footprint.sh"
# The chains entry bundles every chain, so its line sets a larger target.
for entry in index utils; do
  echo "entry-size $entry $(gzip -c dist/$entry.js | wc -c)B" >> "$UPHILL_METRICS"
done
echo "entry-size chains $(gzip -c dist/chains.js | wc -c)B target 15kB" >> "$UPHILL_METRICS"
```

```text
$ uphill
■  81  footprint
  ■  81  entry-size  p90 6.1 kB (index)  target 5 kB
                     6.1 kB  index
                     12 kB   chains  target 15 kB
                     1.1 kB  utils

Opportunities
  +9  footprint  entry-size index  6.1 kB → 5 kB
```

### Override the Script

The project decides that `chains` must stay under 8 kB. Its own target in the baseline file replaces the line target, and `chains` is the worst item again. The `*` key holds the default once the metric lists items.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# The project's target for chains replaces the script's.
footprint:
  entry-size:
    "*": { target: 5kB }
    chains: { target: 8kB }
```

```text
$ uphill
■  70  footprint
  ■  70  entry-size  p90 12 kB (chains)  target 8 kB
                     12 kB   chains  target 8 kB
                     6.1 kB  index
                     1.1 kB  utils

Opportunities
  +11  footprint  entry-size chains  12 kB → 8 kB
```

### Accept the Targets

`uphill --update` writes each item's value beside the target it scored against, unless a pattern or the default already sets that target. An item with its own target keeps it, so a script that changes its line target changes nothing. To take a line's target again, delete the item's target and run `uphill --update`.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# After uphill --update, chains holds its own target, and the default sets the others.
footprint:
  entry-size:
    "*": { target: 5kB }
    chains: { baseline: 12033B, target: 8kB }
    index: { baseline: 6112B }
    utils: { baseline: 1132B }
```

### Read Opportunities

An opportunity is one change and the category points it adds. The change is one item that meets its target, one metric that meets its target, or a whole list when no single item moves the score.

The terminal shows the top 5 opportunities across all categories, and `report.md` shows the top 10. Gains under half a point are left out. A project with two categories lists them together:

```text
Opportunities
  +29  footprint  bundle                     27.5 kB → 5 kB
  +19  speed      render parse               5 ms → 2 ms
  +11  footprint  links about.html           fail → pass
```

## Recipes

### Target by Pattern

A key with `*` and only a target is a pattern. It sets the target of each item without its own, such as a new entry point, and replaces the line target. In a pattern, `*` matches any run of characters, and the first matching pattern applies.

```yaml title=".agents/uphill/uphill.baseline.yaml"
# Each chain entry point scores against 1 kB, and every other entry point against 5 kB.
footprint:
  export-size:
    "*": { target: 5kB }
    "viem/chains/*": { target: 1kB }
```

### Higher Is Better

A metric where more is better, such as test coverage or throughput, sets `better: higher`. Half the target then scores 50, and a list collapses to its p10 item, the one furthest below its target.

```yaml title=".agents/uphill/uphill.yaml"
# Each file needs at least 80% line coverage, with `coverage: { target: 80 }` in the baseline file.
quality:
  run: sh .agents/uphill/metrics/coverage.sh
  coverage: { better: higher }
```

```text
$ uphill
■  78  quality
  ■  78  coverage  p10 61 (src/cache.ts)  target 80
                   61  src/cache.ts
                   88  src/format.ts
                   95  src/parse.ts
```

### Change the Roll-Up

The `score` key picks how a list collapses: `p90`, `p10`, `median`, `max`, `min`, or `sum`. A `sum` scores the total of the values against the total of the targets, so each item with the default target adds that target to the total.

```yaml title=".agents/uphill/uphill.yaml"
# Score the total size of every chunk. Two chunks at `target 25kB` give a total of 50 kB.
footprint:
  run: sh .agents/uphill/metrics/chunks.sh
  chunk-size: { score: sum }
```

```text
$ uphill
■  87  footprint
  ■  87  chunk-size  sum 53 kB  target 50 kB
                     31 kB  main
                     22 kB  vendor
```

### Weight a Metric

A category score is the weighted mean of its metric scores. The `weight` key defaults to 1. A metric with weight 0 still has a score, but the category mean leaves it out.

```yaml title=".agents/uphill/uphill.yaml"
# Throughput counts twice as much as render time.
speed:
  run: sh .agents/uphill/metrics/speed.sh
  render: {}
  throughput: { better: higher, weight: 2 }
```

### Unscored Metrics

A metric without a target in the baseline file shows its value and its changes, but has no score. A check needs no target, because a list of checks scores the share of items that pass.

```yaml title=".agents/uphill/uphill.yaml"
# Without targets, modules has no score, and links scores the share of pages whose links resolve.
footprint:
  run: sh .agents/uphill/metrics/footprint.sh
  modules: {}
  links: {}
```

```text
      –  modules      412
  ■  67  links        2 of 3 pass
                      fail  about.html
```

## More

- [Baselines](./baselines.md): Record accepted values beside their targets.
- [Scoring](../reference/scoring.md): The curve, the bands, and how lists collapse.
- [Configuration](../reference/configuration.md): Every key a metric accepts, and the baseline file.
- [Metric Lines](../reference/metric-lines.md): Write a target on a line.
