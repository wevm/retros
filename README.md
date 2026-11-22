<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#philosophy">Philosophy</a> · <a href="#features">Features</a> · <a href="#scoring">Scoring</a> · <a href="#targets">Targets</a> · <a href="#baselines">Baselines</a> · <a href="#usage">Usage</a>
</p>

## Overview

Retros tracks the numbers a project cares about, such as bundle size, page weight, or request latency. Each number has a target. On every change, Retros shows which numbers improved, which regressed, and what to fix next.

A project lists its metrics in `retros.toml` and measures them with scripts in any language. Each script writes one line per measurement. Retros runs the scripts, scores each metric from 0 to 100, and compares the results with the last accepted run.

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
- [**Item lists**](examples/web-app): thousands of items per metric, such as one per export or page, with the worst named in each report.
- [**Opportunities**](examples/typescript-library): the changes that would raise each category most, with the points each adds.
- [**Baselines**](#baselines): accepted values in `retros.lock`, and a list of every change since.
- [**CI gate**](#baselines): `--require-baseline` fails on any change that is not accepted, better or worse.
- [**Reports**](#usage): terminal output, `.retros/report.json`, and `.retros/report.md`.

## Scoring

A metric scores 90 at its target, 50 at twice the target, and 10 at four times the target. Scores follow Lighthouse's log-normal curve and bands: 90 to 100 is good, 50 to 89 needs work, and 0 to 49 is poor.

| Value, against a 5 kB target | Score |
| --- | --- |
| 2.5 kB | 99 |
| 5 kB | 90 |
| 10 kB | 50 |
| 20 kB | 10 |

A metric with many items scores one of them. Retros ranks the items by how far each is past its own target, then scores the item at the 90th percentile. A check scores the share of items that pass.

A category score is the weighted mean of its metric scores. When a script writes a time, Retros runs the script 5 times and scores the median of each item.

## Targets

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

## Baselines

`retros --update` writes `retros.lock`. The lock records every size, count, and check, and each target a script wrote on a line. Commit it. Later runs list what got worse or better, what is new or gone, and which targets changed.

```toml
# retros.lock: a line target follows its value, so loosening one shows in review.
[footprint.export-size]
"viem" = "1000B target 2000B"
"viem/chains" = "10000B"
```

`retros --require-baseline` exits with status 1 when anything differs from the lock, improvements included. To accept a change, commit the updated lock. Timings stay out of the lock because they vary between machines.

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

- **Metric line:** `metric [item] value [target <value>]`. Quote an item that contains a space.
- **Values:** sizes in `B`, `kB`, `MB`, or `GB`, times in `ns`, `µs`, `ms`, or `s`, counts without a unit, and checks as `pass` or `fail`.
- **Category keys:** `run`, and `timeout` (default `10m`).
- **Metric keys:** `target`, `targets`, `weight` (default 1), `score` (`p90` by default, or `p10`, `median`, `max`, `min`, `sum`), and `better`.

[`examples/`](examples) has a TypeScript library, a website measured by Lighthouse, and a Rust API measured with Cargo's tools. Retros measures itself with the root [`retros.toml`](retros.toml).

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
