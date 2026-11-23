<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#philosophy">Philosophy</a> · <a href="#usage">Usage</a>
</p>

## Overview

Retros scores a project's metrics against targets, locally or in CI. Scripts in any language take the measurements. Retros scores each metric from 0 to 100, compares it with the last accepted run, and names the fixes worth the most points.

## Problem

Most projects check speed and size now and then, with separate tools. Regressions land between checks, and improvements wear away. Without a number to compare, a person or an agent cannot tell a better change from a worse one.

## Solution

Retros shows whether a change helped. A script measures each metric with tools the project already uses, and Retros scores the result against its target. The top opportunity is the next step up.

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
