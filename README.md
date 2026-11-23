<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#install">Install</a> · <a href="#usage">Usage</a> · <a href="#github-action">GitHub Action</a>
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

## Install

```sh
# Install the latest release to ~/.retros/bin on macOS or Linux.
curl -fsSL https://raw.githubusercontent.com/wevm/retros/main/install.sh | sh

# Or run the latest release with npx.
npx retros

# Or build it from source with Cargo.
cargo install --locked --git https://github.com/wevm/retros retros
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

# Compare with another revision: timings in interleaved pairs, and exact values with its baselines.
retros --compare main

# Run each script once and check its lines, without scoring.
retros check

# Print a brief for an agent to set up Retros in this project.
retros init

# Print the scores, missed targets, and opportunities in a few lines, for agent loops.
retros --compact

# Keep this run as a Git note on HEAD, for trends in report.html and `retros log`.
retros --note

# List the commits that changed a metric's baseline, or the runs noted for it.
retros log footprint bundle

# Name the commit that last moved a metric, bisecting commits when no baseline records it.
retros blame speed startup
```

## GitHub Action

The action runs `retros --compare` against the base branch of each pull request, and keeps one comment on the pull request with `report.md`. The job summary also has the report, and the job fails when `retros` fails.

```yaml
# .github/workflows/retros.yml: score each pull request and compare it with its base branch.
name: Retros
on: pull_request

jobs:
  retros:
    permissions:
      contents: read
      pull-requests: write
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      # Install the tools the scripts run here, such as Bun.
      - uses: wevm/retros@main
        with:
          args: --require-targets
```

## License

[MIT](LICENSE)
