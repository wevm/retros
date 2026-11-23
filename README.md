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

> [!NOTE]
> The first release is not published yet, so only the Cargo command works for now.

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

- **Metric line:** `[subject:] metric [item] value [target <value>] [note <text>]`. Quote an item or note that contains a space. A note says why, such as `alt-text /pricing fail note "hero.svg has no alt text"`, and every report shows it under its item.
- **Values:** sizes in `B`, `kB`, `MB`, or `GB`, times in `ns`, `µs`, `ms`, or `s`, counts without a unit, and checks as `pass` or `fail`.
- **Category keys:** `run`, `timeout` (default `10m`), `title`, and `description`. A top-level `run` and `timeout` apply to categories that set none.
- **Setup:** a top-level `setup` command, such as `bun install --frozen-lockfile`, prepares each checkout that `--compare` and `blame` create.
- **Metric keys:** `target`, `targets`, `weight` (default 1), `score` (`p90` by default, or `p10`, `median`, `max`, `min`, `sum`), `better`, `varies`, `title`, `description`, and `baseline`.
- **Baselines:** `retros --update` writes each metric's `baseline` into the configuration, one line per item. Commit them, so each run reports what changed.
- **Comparisons:** `--compare <ref>` checks out `<ref>` in a temporary Git worktree. Timings change only when the medians differ by at least 5% and 7 of 8 pairs agree.
- **History:** `retros --note` keeps a run under `refs/notes/retros`, and `git push origin refs/notes/retros` shares it. `report.html` draws trends from the last 30 noted runs. `retros blame` measures commits in temporary worktrees when no baseline records the metric. Each commit runs the `run` command its own configuration declares, so a moved script still runs. `--since <ref>` limits the commits.
- **Files:** reports go to `.retros/` in the project root.

[`examples/`](examples) has a TypeScript library, a website, and a Rust API. Retros measures itself with [`.agents/retros.yaml`](.agents/retros.yaml).

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

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

- **`args`:** more arguments for `retros`, such as a category or `--require-targets`.
- **`compare`:** the branch, tag, or commit to compare with. It defaults to the base branch, which the action fetches when the clone lacks it. An empty value skips the comparison.
- **`comment`:** `false` keeps the report in the job summary only.
- **`version`:** the release to install. It defaults to the action's tag, or the latest release. `none` uses the `retros` on `PATH`.
- **`working-directory`:** the project root.

## License

[MIT](LICENSE)
