<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#philosophy">Philosophy</a> · <a href="#features">Features</a> · <a href="#usage">Usage</a>
</p>

## Overview

Software improves fastest when every change has a number to beat. Each attempt is measured, the attempts that climb stay, and the rest are dropped. This loop is hill climbing. Agents run it well, because they make and measure attempts far faster than people do.

The climb needs continuous metrics. A metric measured on every change names the change that moved it. A metric measured once before a release shows only that something moved, long after the change that caused it.

Retros runs that loop for a project. It tracks numbers such as bundle size, page weight, and request latency, and scores each one against a target on every change. Each report names what improved, what regressed, and what to fix next.

## Problem

Projects measure speed and size in scattered places, each tool with its own format and none with a view of the whole. Regressions land unnoticed and improvements erode. A single limit for every item is either too loose for small items or always red for large ones.

## Solution

Scripts in any language run the tools a project already trusts and write one line per measurement. Retros scores each metric from 0 to 100 against its target and compares the run with the last accepted one. Each report names the changes that would raise the scores most.

```toml
# retros.toml: the `footprint` category runs one script and expects one metric.
[footprint]
run = "sh footprint.sh"
entry-size = { target = "1kB" }
```

```sh
# footprint.sh: one line per entry point, appended to the file Retros provides.
echo 'entry-size ./date 247B' >> "$RETROS_METRICS"
echo 'entry-size ./number 1.6kB' >> "$RETROS_METRICS"
```

```text
■  66  footprint
  ■  66  entry-size  p90 1.6 kB (./number)  target 1 kB
                     1.6 kB  ./number
                     247 B   ./date

Opportunities
  +24  footprint  entry-size ./number  1.6 kB → 1 kB
```

## Philosophy

- **A target is a requirement:** A value at its target scores 90, and anything worse costs points. No second threshold makes a miss acceptable.
- **Items differ:** A root entry point, a landing page, and a health check each need their own target. Retros scores every item against the target that fits it.
- **Any project:** Retros measures nothing itself. Scripts run the tools a project already trusts, such as Lighthouse, a test runner, or a linter, and write what they report. A TypeScript library, a website, and a Rust server all use the same text format.
- **Progress is kept:** Once accepted, a better value becomes the new baseline. A regression needs a reviewed change to `retros.lock`.

## Features

- [**Scores**](#scoring): a score from 0 to 100 for every metric, and a weighted mean for every category.
- [**Contextual targets**](#targets): a default per metric, a target per line from the script, and item patterns that override both.
- [**Item lists**](#scoring): thousands of items per metric, such as one per export or page, with the worst named in each report.
- [**Subjects**](#subjects): several harnesses, packages, or builds measured by one script, each scored on its own and compared side by side.
- [**Opportunities**](#opportunities): the changes that would raise each category most, with the points each adds.
- [**Baselines**](#baselines): accepted values in `retros.lock`, and a list of every change since.
- [**CI gate**](#baselines): `--require-baseline` fails on any change that is not accepted, better or worse.
- [**Reports**](#reports): terminal output, a self-contained HTML report, and JSON and Markdown beside it.

### Scoring

A metric scores 90 at its target, 50 at twice the target, and 10 at four times the target. Scores follow Lighthouse's log-normal curve and bands: 90 to 100 is good, 50 to 89 needs work, and 0 to 49 is poor.

| Value, against a 5 kB target | Score |
| --- | --- |
| 2.5 kB | 99 |
| 5 kB | 90 |
| 10 kB | 50 |
| 20 kB | 10 |

A metric with many items scores one of them. Retros ranks the items by how far each is past its own target, then scores the item at the 90th percentile. A check scores the share of items that pass.

A category score is the weighted mean of its metric scores. When a script writes a time, Retros runs the script 5 times and scores the median of each item. A size or count marked `varies`, such as peak memory, takes its median too.

### Targets

A target comes from one of three places. Each one overrides the one before it.

- **`target`:** The default in `retros.toml` for every item of the metric.
- **Line target:** A script ends a line with `target <value>` when it knows that item needs a different requirement.
- **`targets`:** Item patterns in `retros.toml`. `*` matches any run of characters.

```toml
# Entry points default to 1 kB. The root re-exports every module, so it gets 3 kB.
entry-size = { target = "1kB", targets = { "." = "3kB" } }

# Higher is better for throughput, so half the target scores 50.
throughput = { target = 5000, better = "higher" }
```

### Subjects

A line can start with a subject, such as a harness, a package, or a build. Retros scores each subject on its own and shows them side by side. A top-level `run` measures every category that sets none, and categories with the same `run` share one execution.

```toml
# One script measures both builds. Each category scores the metrics it lists.
run = "bun metrics/builds.ts"

[speed]
startup = { target = "30ms" }

[footprint]
peak-memory = { target = "16MB", varies = true }
```

```sh
# The Node build runs on an engine that holds 41 MB at rest, so its line sets its own target.
echo 'native: startup 0.64ms' >> "$RETROS_METRICS"
echo 'native: peak-memory 2.4MB' >> "$RETROS_METRICS"
echo 'node: startup 31.9ms' >> "$RETROS_METRICS"
echo 'node: peak-memory 48.4MB target 57MB' >> "$RETROS_METRICS"
```

### Opportunities

Retros lists the changes that would raise each category most, in category points. Each change is one item or one metric that meets its target, or a whole list when no single item moves the score.

```text
Opportunities
  +6  zeptocode-ts: speed  request-out  32.8 ms → 20 ms
  +2  zc: scale            step-time    5.4 ms → 5 ms
```

### Baselines

`retros --update` writes `retros.lock`. The lock records every size, count, and check, and each target a script wrote on a line. Commit it. Later runs list what got worse or better, what is new or gone, and which targets changed.

```toml
# retros.lock: a line target follows its value, so loosening one shows in review.
[footprint.export-size]
"viem" = "1000B target 2000B"
"viem/chains" = "10000B"
```

`retros --require-baseline` exits with status 1 when anything differs from the lock, improvements included. To accept a change, commit the updated lock. Timings stay out of the lock because they vary between machines.

### Reports

Each run prints every category as it finishes and writes three files to `.retros/`. `report.html` is a self-contained page with a gauge per category and a tab per subject. `report.json` is for tools, and `report.md` is for pull requests and CI summaries.

```sh
# Show the scores in a GitHub Actions job summary.
retros
cat .retros/report.md >> "$GITHUB_STEP_SUMMARY"
```

## Usage

Run Retros from the project or any directory below it, and add `.retros/` to `.gitignore`.

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

[`examples/`](examples) has a TypeScript library, a website measured by Lighthouse, and a Rust API measured with Cargo's tools. Retros measures itself with the root [`retros.toml`](retros.toml).

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
