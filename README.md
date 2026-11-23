<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#usage">Usage</a>
</p>

## Overview

Retros is Lighthouse for your project. Lighthouse scores a web page and lists the fixes that would raise each score. Retros brings the same loop to any codebase, with the metrics that project cares about: bundle size, request latency, memory, test coverage, or anything a script can measure.

Each metric has a target, and each run scores the project from 0 to 100 against it. The scores show where the project stands, the opportunities show the next step up, and `retros.lock` keeps every gain.

## Problem

Quality and performance slip a little at a time. A bundle grows by a few hundred bytes, an endpoint slows by a millisecond, a lint warning stays. No single change looks like the cause, so nobody notices until users do.

## Solution

Retros turns those numbers into targets and scores. Scripts measure with the tools the project already uses, and Retros scores each metric and names the fix worth the most points. Run it in CI or in an agent's loop, and the project climbs toward its targets instead of drifting away.

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

Retros reads `retros.yaml` or `.agents/retros.yaml` from the current directory or the nearest parent. Scripts run from the project root, the directory that holds `retros.yaml` or `.agents`. Add `.retros/` to `.gitignore`.

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
- **Files:** `retros.lock` sits beside `retros.yaml`. Reports go to `.retros/` in the project root.

[`examples/`](examples) has a TypeScript library, a website measured with Lighthouse, and a Rust API measured with Cargo. Retros measures itself with [`.agents/retros.yaml`](.agents/retros.yaml).

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
