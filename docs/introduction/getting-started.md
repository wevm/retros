# Getting Started

Uphill scores a project against its own targets. Scripts measure the project, Uphill scores each metric from 0 to 100, and each run names the fixes worth the most points.

## Install

Uphill is one binary for macOS and Linux. [Installation](./installation.md) lists every way to install it.

```sh
# Install the latest release to ~/.uphill/bin/uphill.
curl -fsSL https://raw.githubusercontent.com/wevm/uphill/main/install.sh | sh
```

## Add a Category

A category groups metrics that one script measures. `uphill.yaml` declares each category, its `run` command, and a target for each metric. Uphill reads it from the project root or from `.agents/`.

```yaml title=".agents/uphill.yaml"
# The footprint category runs one script, which writes the bundle-size metric.
footprint:
  run: sh .agents/metrics/footprint.sh
  bundle-size: { target: 5kB }
```

## Write a Script

A script measures with the tools the project already uses. It appends one metric line per measurement to the file at `$UPHILL_METRICS`. Standard output and standard error are logs.

```sh title=".agents/metrics/footprint.sh"
# Measure the gzipped bundle and write one metric line.
echo "bundle-size $(gzip -c dist/index.js | wc -c)B" >> "$UPHILL_METRICS"
```

## Check the Script

`uphill check` runs each script once and checks the lines it writes. A misspelled metric, an unknown unit, or a metric that the script does not write fails the check with its file, line, and column.

```text
$ uphill check
✓ footprint  1 metric

1 passed
```

## Score the Project

`uphill` runs every category, scores each metric against its target, and lists opportunities: the changes that raise a category score most. A value at its target scores 90.

```text
$ uphill
■  75  footprint
  ■  75  bundle-size  6.9 kB  target 5 kB

Opportunities
  +15  footprint  bundle-size  6.9 kB → 5 kB

No baseline yet. Run `uphill --update` to record these values as the baseline.

Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
```

Each run writes three reports to `.uphill/` in the project root. `report.html` is one file that opens in a browser without network requests. Add `.uphill/` to `.gitignore`.

## Record a Baseline

`uphill --update` writes each metric's accepted value to its `baseline` key. Commit the configuration, and every later run reports what changed since then.

```yaml title=".agents/uphill.yaml"
# uphill --update added the baseline key.
footprint:
  run: sh .agents/metrics/footprint.sh
  bundle-size: { target: 5kB, baseline: 6.9kB }
```

## Agent Setup

`uphill init` prints a brief that a person gives their agent. The brief asks the agent to find the project's surfaces, write a script per category with the project's own tools, and run `uphill check` until it passes. `uphill --compact` then prints the next step in a few lines.

```sh
# Print the setup brief for this project.
uphill init
```

## More

- [Writing Scripts](../guides/scripts.md): Write metric lines for lists, subjects, timings, and checks.
- [Targets](../guides/targets.md): Set targets per item and read opportunities.
- [Baselines](../guides/baselines.md): Record accepted values and gate on changes.
- [GitHub Actions](../guides/github-actions.md): Compare pull requests and keep one comment.
