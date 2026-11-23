<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#usage">Usage</a>
</p>

## Overview

Retros scores a project against its own targets. A metric is anything a script can measure: bundle size, page load time, request latency, memory, test coverage, lint warnings, or an eval score. Retros runs the scripts, scores each metric from 0 to 100, and names the fixes worth the most points.

## Problem

Most projects track their numbers in one-off scripts and CI checks that pass or fail. A check says whether a limit broke, not how far the project is from its target or which fix would help most. Regressions slip under loose limits, and improvements fade because nothing records them.

## Solution

Retros turns every metric into a score and every gap into a next step. Scripts measure with the tools the project already uses. Each run shows where the project stands, so a person or an agent can climb toward the targets one fix at a time.

```yaml
# 1. .agents/retros.yaml: the footprint category, its script, and its target.
footprint:
  run: sh .agents/metrics/footprint.sh
  bundle-size: { target: 5kB }
```

```sh
# 2. .agents/metrics/footprint.sh: measure the gzipped bundle and write one metric line.
echo "bundle-size $(gzip -c dist/index.js | wc -c)B" >> "$RETROS_METRICS"
```

```text
$ retros
■  76  footprint
  ■  76  bundle-size  6.9 kB  target 5 kB

Opportunities
  +14  footprint  bundle-size  6.9 kB → 5 kB
```

## Usage

Retros reads `retros.yaml` or `retros.toml`, at the project root or in `.agents/`, from the current directory or the nearest parent. Scripts run from the project root. Add `.retros/` to `.gitignore`.

```sh
# Score every category, list opportunities, and compare with each metric's baseline.
retros

# Score one category.
retros speed

# Accept this run: write its values and line targets to each metric's baseline.
retros --update

# Fail when anything differs from the baselines.
retros --require-baseline

# Fail when a metric misses its target or a script fails.
retros --require-targets

# Run each script once and check its lines, without scoring.
retros check
```

- **Metric line:** `[subject:] metric [item] value [target <value>]`. Quote an item that contains a space.
- **Values:** sizes in `B`, `kB`, `MB`, or `GB`, times in `ns`, `µs`, `ms`, or `s`, counts without a unit, and checks as `pass` or `fail`.
- **Category keys:** `run`, `timeout` (default `10m`), `title`, and `description`. A top-level `run` and `timeout` apply to categories that set none.
- **Metric keys:** `target`, `targets`, `weight` (default 1), `score` (`p90` by default, or `p10`, `median`, `max`, `min`, `sum`), `better`, `varies`, `title`, `description`, and `baseline`.
- **Baselines:** `retros --update` writes each metric's `baseline` into the configuration, one line per item. Commit them, so each run reports what changed.
- **Files:** reports go to `.retros/` in the project root.

[`examples/`](examples) has a TypeScript library, a website, and a Rust API. Retros measures itself with [`.agents/retros.yaml`](.agents/retros.yaml).

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
