<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#philosophy">Philosophy</a> · <a href="#features">Features</a> · <a href="#usage">Usage</a>
</p>

## Overview

Retros scores a project's metrics against targets on every change. Scripts in any language take the measurements. Retros scores each metric from 0 to 100, compares it with the last accepted run, and names the fixes worth the most points.

## Problem

Most projects check speed and size now and then, with separate tools. Regressions land between checks, and improvements wear away. Without a number on every change, a person or an agent cannot tell a better change from a worse one.

## Solution

Retros puts a score on every change. A script measures each metric with tools the project already uses, and Retros scores the result against its target. The top opportunity is the next step up.

```toml
# 1. Declare the metric, its target, and the script that measures it.
[footprint]
run = "sh footprint.sh"
bundle-size = { target = "5kB" }
```

```sh
# 2. Measure the gzipped bundle and write one metric line.
echo "bundle-size $(gzip -c dist/index.js | wc -c)B" >> "$RETROS_METRICS"
```

```text
$ retros
■  76  footprint
  ■  76  bundle-size  6.9 kB  target 5 kB

Opportunities
  +14  footprint  bundle-size  6.9 kB → 5 kB
```

## Philosophy

- **Targets are requirements:** A value at its target scores 90. Anything worse loses points.
- **Items differ:** A root entry point and a small subpath need different targets. Each item gets its own.
- **Any project:** Retros measures nothing itself. Scripts run Lighthouse, a test runner, a linter, or any other tool, and write what it reports.
- **Progress sticks:** An accepted improvement becomes the new baseline. Accepting a regression takes a reviewed change to `retros.lock`.

## Features

- [**Scores**](#scoring): 0 to 100 for each metric and category.
- [**Item targets**](#targets): `3kB` for the root, `1kB` elsewhere.
- [**Item lists**](#scoring): thousands of exports or pages, worst first.
- [**Subjects**](#subjects): native and Node builds, scored side by side.
- [**Opportunities**](#opportunities): the next fix and the points it adds.
- [**Baselines**](#baselines): accepted values, committed in `retros.lock`.
- [**CI gate**](#baselines): fails on any change not in `retros.lock`.
- [**Reports**](#reports): terminal, HTML, JSON, and Markdown.

### Scoring

A metric scores 90 at its target, 50 at twice the target, and 10 at four times it. The curve and the bands come from Lighthouse: 90 to 100 is good, 50 to 89 needs work, and 0 to 49 is poor.

| Value, against a 5 kB target | Score |
| --- | --- |
| 2.5 kB | 99 |
| 5 kB | 90 |
| 10 kB | 50 |
| 20 kB | 10 |

A list of items scores one item, the 90th percentile by default, ranked by how far each item is past its own target. A check scores the share of items that pass.

A category scores the weighted mean of its metrics. A script that writes a time runs 5 times, and each item scores its median. A size or count marked `varies` takes its median too.

### Targets

Each source of a target overrides the one before it.

- **`target`:** The metric's default in `retros.toml`.
- **Line target:** `target <value>` at the end of a script's line.
- **`targets`:** Item patterns in `retros.toml`, where `*` matches any run of characters.

```toml
# The root re-exports every module, so it gets 3 kB. Other entry points get 1 kB.
entry-size = { target = "1kB", targets = { "." = "3kB" } }

# Higher is better for throughput, so half the target scores 50.
throughput = { target = 5000, better = "higher" }
```

### Subjects

A line can start with a subject, such as a build or a harness. Each subject is scored on its own. A top-level `run` covers every category without one, and categories with the same `run` share one execution.

```toml
# One script measures both builds.
run = "bun metrics/builds.ts"

[speed]
startup = { target = "30ms" }

[footprint]
peak-memory = { target = "16MB", varies = true }
```

```sh
# Node holds 41 MB at rest, so the Node build sets its own memory target.
echo 'native: startup 0.64ms' >> "$RETROS_METRICS"
echo 'native: peak-memory 2.4MB' >> "$RETROS_METRICS"
echo 'node: startup 31.9ms' >> "$RETROS_METRICS"
echo 'node: peak-memory 48.4MB target 57MB' >> "$RETROS_METRICS"
```

### Opportunities

Retros lists the fixes that would raise each category most, in points. A fix is one item or metric that meets its target, or a whole list when no single item moves the score.

```text
Opportunities
  +6  zeptocode-ts: speed  request-out  32.8 ms → 20 ms
  +2  zc: scale            step-time    5.4 ms → 5 ms
```

### Baselines

`retros --update` records every size, count, check, and line target in `retros.lock`. Commit it. Later runs list each change since, item by item.

```toml
# retros.lock: a line target follows its value, so a loosened target shows in review.
[footprint.export-size]
"viem" = "1000B target 2000B"
"viem/chains" = "10000B"
```

`retros --require-baseline` exits with status 1 when anything differs from the lock, improvements included. Times and `varies` values stay out of the lock because they change between runs.

### Reports

Each run writes `report.html`, `report.json`, and `report.md` to `.retros/`. The HTML page has a gauge per category and a tab per subject.

```sh
# Show the scores in a GitHub Actions job summary.
retros
cat .retros/report.md >> "$GITHUB_STEP_SUMMARY"
```

## Usage

Run Retros from the project or any directory below it. Add `.retros/` to `.gitignore`.

```sh
# Score every category, list opportunities, and compare with retros.lock.
retros

# Score one category.
retros speed

# Accept this run: record its values and line targets in retros.lock.
retros --update

# Fail when anything differs from retros.lock.
retros --require-baseline

# Run each script once and check its lines, without scoring.
retros check
```

- **Metric line:** `[subject:] metric [item] value [target <value>]`. Quote an item that contains a space.
- **Values:** sizes in `B`, `kB`, `MB`, or `GB`, times in `ns`, `µs`, `ms`, or `s`, counts without a unit, and checks as `pass` or `fail`.
- **Category keys:** `run`, `timeout` (default `10m`), and `description`. A top-level `run` and `timeout` apply to categories that set none.
- **Metric keys:** `target`, `targets`, `weight` (default 1), `score` (`p90` by default, or `p10`, `median`, `max`, `min`, `sum`), `better`, `varies`, and `description`.

[`examples/`](examples) has a TypeScript library, a website measured with Lighthouse, and a Rust API measured with Cargo. Retros measures itself with the root [`retros.toml`](retros.toml).

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
