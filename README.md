<h1 align="center">Retros</h1>

<p align="center">
  Lighthouse scores for any project
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#philosophy">Philosophy</a> · <a href="#scoring">Scoring</a> · <a href="#status">Status</a>
</p>

## Overview

A project's scripts measure it. Retros scores the metrics they write, compares revisions, and writes reports.

```toml
# retros.toml: each table is a category, and `run` is the script that measures it.
[footprint]
run = "node metrics/footprint.ts"
export-size = { target = "5kB", limit = "50kB" }
install-size = { target = "10MB", limit = "150MB" }
```

```sh
# A script appends one line per measurement to the file at $RETROS_METRICS.
echo 'install-size 45.6MB' >> "$RETROS_METRICS"
echo 'export-size viem/actions:readContract 31.7kB' >> "$RETROS_METRICS"
```

## Philosophy

- **Scripted.** Projects measure themselves with scripts in any language. Retros runs them and reads one line per measurement.
- **Scored.** Every metric with anchors scores from 0 to 100 on Lighthouse's curve, and each category averages its metrics.
- **Comparable.** Sizes and counts report every change. Timings report a change only when repeated, interleaved runs agree.
- **Portable.** One static binary, with no runtime to install.

## Scoring

A value at the `target` anchor scores 90, the start of the good band, and a value at the `limit` anchor scores 50, the start of the poor band. A metric that covers a list of items, such as every export, scores its p90. Bands follow Lighthouse: 90 to 100 is good, 50 to 89 needs work, and 0 to 49 is poor.

## Status

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
