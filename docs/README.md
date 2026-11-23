# Documentation

Uphill scores a project against its own targets. Scripts measure the project, Uphill scores each metric from 0 to 100, and each run names the fixes worth the most points. The pages below go from a first run to every key and flag.

## Introduction

- [Getting Started](./introduction/getting-started.md): Install Uphill, write a script, and score a project.
- [Installation](./introduction/installation.md): The install script, npm, Cargo, and the release archives.

## Guides

- [Writing Scripts](./guides/scripts.md): Write metric lines for lists, subjects, timings, and checks.
- [Targets](./guides/targets.md): Set a target for each item and read opportunities.
- [Baselines](./guides/baselines.md): Record accepted values and gate on changes.
- [Comparing Revisions](./guides/comparing.md): Measure another revision and report what changed.
- [GitHub Actions](./guides/github-actions.md): Compare pull requests and keep one comment.

## Reference

- [Configuration](./reference/configuration.md): Every key in `uphill.yaml` and `uphill.toml`.
- [Metric Lines](./reference/metric-lines.md): The line format that scripts write, and its errors.
- [Units](./reference/units.md): Sizes, times, counts, and checks.
- [Scoring](./reference/scoring.md): The curve, the bands, and how lists collapse.
- [Reports](./reference/reports.md): The summary, the JSON, Markdown, and HTML reports, and the dashboard.
- [CLI](./reference/cli.md): Every command and flag of the `uphill` binary.
- [GitHub Action](./reference/action.md): The inputs and outputs of `wevm/uphill`.
