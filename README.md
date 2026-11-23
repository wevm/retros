<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#usage">Usage</a>
</p>

## Overview

Retros is Lighthouse for your project. It scores the metrics a project chooses, such as bundle size or request latency, against targets, and lists the fixes that would raise each score.

## Problem

Lighthouse only scores web pages. A library, an API, or a CLI has no equivalent. Its numbers end up in one-off scripts and CI checks that pass or fail, with no score and no list of what to fix next.

## Solution

Retros gives any project the same loop. Scripts measure with the tools the project already uses, Retros scores each metric against its target, and the top opportunity is the next fix to make.

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
- **Files:** `retros.lock` sits beside the configuration. Reports go to `.retros/` in the project root.

[`examples/`](examples) has a TypeScript library, a website measured with Lighthouse, and a Rust API measured with Cargo. Retros measures itself with [`.agents/retros.yaml`](.agents/retros.yaml).

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
