<h1 align="center">Uphill</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#problem">Problem</a> · <a href="#solution">Solution</a> · <a href="#install">Install</a> · <a href="#usage">Usage</a> · <a href="#github-action">GitHub Action</a>
</p>

## Overview

Uphill scores a project against its own targets. A metric is anything a script can measure: bundle size, page load time, request latency, memory, test coverage, lint warnings, or an eval score. Uphill runs the scripts, scores each metric from 0 to 100, and names the fixes worth the most points.

## Problem

Most projects track their numbers in one-off scripts and CI checks that pass or fail. A check says whether a limit broke, not how far the project is from its target or which fix would help most. Regressions slip under loose limits, and improvements fade because nothing records them.

## Solution

Uphill turns every metric into a score and every gap into a next step. Scripts measure with the tools the project already uses. Each run shows where the project stands, so a person or an agent can climb toward the targets one fix at a time.

```yaml
# 1. .agents/uphill.yaml: the footprint category, its script, and its target.
footprint:
  run: sh .agents/metrics/footprint.sh
  bundle-size: { target: 5kB }
```

```sh
# 2. .agents/metrics/footprint.sh: measure the gzipped bundle and write one metric line.
echo "bundle-size $(gzip -c dist/index.js | wc -c)B" >> "$UPHILL_METRICS"
```

```text
$ uphill
■  76  footprint
  ■  76  bundle-size  6.9 kB  target 5 kB

Opportunities
  +14  footprint  bundle-size  6.9 kB → 5 kB
```

## Install

```sh
# Install the latest release to ~/.uphill/bin on macOS or Linux.
curl -fsSL https://raw.githubusercontent.com/wevm/uphill/main/install.sh | sh

# Or run the latest release with npx.
npx uphill

# Or build it from source with Cargo.
cargo install --locked --git https://github.com/wevm/uphill uphill
```

## Usage

Uphill reads `uphill.yaml` or `uphill.toml`, at the project root or in `.agents/`, from the current directory or the nearest parent. Scripts run from the project root. Add `.uphill/` to `.gitignore`.

```sh
# Score every category, list opportunities, and compare with each metric's baseline.
uphill

# Score one category.
uphill speed

# Accept this run: write its values and line targets to each metric's baseline.
uphill --update

# Fail when anything differs from the baselines.
uphill --require-baseline

# Fail when a metric misses its target or a script fails.
uphill --require-targets

# Compare with another revision: timings in interleaved pairs, and exact values with its baselines.
uphill --compare main

# Run each script once and check its lines, without scoring.
uphill check

# Print a brief for an agent to set up Uphill in this project.
uphill init

# Print the scores, missed targets, and opportunities in a few lines, for agent loops.
uphill --compact

# Keep this run as a Git note on HEAD, for trends in report.html and `uphill log`.
uphill --note

# List the commits that changed a metric's baseline, or the runs noted for it.
uphill log footprint bundle

# Name the commit that last moved a metric, bisecting commits when no baseline records it.
uphill blame speed startup
```

## GitHub Action

The action runs `uphill --compare` against the base branch of each pull request, and keeps one comment on the pull request with `report.md`. The job summary also has the report, and the job fails when `uphill` fails.

```yaml
# .github/workflows/uphill.yml: score each pull request and compare it with its base branch.
name: Uphill
on: pull_request

jobs:
  uphill:
    permissions:
      contents: read
      pull-requests: write
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      # Install the tools the scripts run here, such as Bun.
      - uses: wevm/uphill@main
        with:
          args: --require-targets
```

## License

[MIT](LICENSE)
