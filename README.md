<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing for any project
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#philosophy">Philosophy</a> · <a href="#scoring">Scoring</a> · <a href="#status">Status</a>
</p>

## Overview

Set a target for each metric that matters, measure every change, and climb. A project's scripts measure it, and Retros scores each metric against its target, compares revisions, and reports what moved.

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

- **Targeted.** Each metric has a `target` to reach and a `limit` to stay inside. Every change shows whether the project climbed toward its targets or slid back.
- **Scripted.** Projects measure themselves with scripts in any language. Retros runs them and reads one line per measurement.
- **Scored.** Every metric with anchors scores from 0 to 100 on Lighthouse's curve, and each category averages its metrics.
- **Comparable.** Sizes and counts report every change. Timings report a change only when repeated, interleaved runs agree.
- **Portable.** One static binary, with no runtime to install.

## Scoring

A value at the `target` anchor scores 90, the start of the good band, and a value at the `limit` anchor scores 50, the start of the poor band. A metric that covers a list of items, such as every export, scores its p90. Bands follow Lighthouse: 90 to 100 is good, 50 to 89 needs work, and 0 to 49 is poor.

## Status

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

Scoring and reports work today. Run Retros from the project or any directory below it, and add `.retros/` to `.gitignore`.

```sh
# Score every category and write .retros/report.json and .retros/report.md.
retros

# Score one category.
retros speed

# Run each script once and check what it writes, without scoring.
retros check
```

```text
●  95  footprint
  ●  95  entry-size  p90 525 B  target 1 kB, limit 10 kB
                     525 B  .
                     314 B  ./number
                     274 B  ./date

■  61  speed  median of 3 rounds
  ■  61  import-time  p90 6.1 ms  target 1 ms, limit 10 ms
                      6.1 ms  ./number
                      5.1 ms  .
                      4 ms    ./date
```

[`examples/`](examples) has small projects measured with shell, Node, and Python scripts. Retros also measures itself with the root [`retros.toml`](retros.toml).

## License

[MIT](LICENSE)
