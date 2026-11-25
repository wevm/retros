# GitHub Actions

The Uphill action scores each pull request, compares it with its base branch, and keeps one comment on the pull request with the report.

## Overview

The action installs Uphill, runs `uphill --compare` against the base branch, and writes `report.md` to the job summary and to a comment. Each later run updates the same comment. The job fails when `uphill` fails.

```yaml title=".github/workflows/uphill.yml"
# Score each pull request and compare it with its base branch.
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
      - uses: wevm/uphill@main
```

## Walkthrough

This walkthrough adds the action to a project whose scripts run on Bun, and gates each pull request on its targets.

### Add the Workflow

The job needs `pull-requests: write` to write the comment, and `contents: read` to clone the repository. The clone needs only the head commit, because the action fetches the base branch when the clone lacks it.

```yaml title=".github/workflows/uphill.yml"
# 1. Run on each pull request, with permission to comment.
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
```

### Install the Tools

Scripts run the tools the project already uses, so the job installs them before the action runs. The base branch runs in a new worktree, so a top-level `setup` command in `uphill.yaml` installs its packages.

```yaml title=".github/workflows/uphill.yml"
      # 2. Install Bun and the project's packages for the scripts.
      - uses: oven-sh/setup-bun@v2
      - run: bun install --frozen-lockfile
```

```yaml title="uphill.yaml"
# Install packages in the base branch's worktree too.
setup: bun install --frozen-lockfile
```

### Run Uphill

The `args` input adds arguments to `uphill`. `--require-targets` fails the job when a metric misses its target or a script fails.

```yaml title=".github/workflows/uphill.yml"
      # 3. Compare with the base branch, comment, and fail on a missed target.
      - uses: wevm/uphill@main
        with:
          args: --require-targets
```

### Read the Comment

The comment holds `report.md`: scores, opportunities, changes since the baselines, and changes against the base branch. A failed run adds the exit status and the end of the output.

The last line names the command, the commit, and the workflow run. This excerpt leaves out the sections between the tables:

```md
<!-- The next line is the marker that the action finds the comment by. -->
<!-- uphill: . -->
# Hill climb report

| Category | Score |
| --- | --- |
| Footprint | ● 100 |
| Speed | ■ 51 |

## Changes against origin/main (ac66366)

| Change | Category | Metric | Before | After | Pairs |
| --- | --- | --- | --- | --- | --- |
| worse | Footprint | Source size | 138 B | 172 B |  |
| worse | Speed | Slugify time | 550 ns | 984 ns (+79%) | 8 of 8 pairs |

> [!CAUTION]
> `uphill --compare origin/main --require-targets` exited with status 1.

<details><summary>Output</summary>
...
</details>

<sub>`uphill --compare origin/main --require-targets` at fa8deea in [run 12](https://github.com/acme/slugify/actions/runs/1)</sub>
```

## Recipes

### Gate on Baselines

`--require-baseline` fails the job when a size, count, or check differs from its baseline, so each change to them comes with an `uphill --update`.

```yaml title=".github/workflows/uphill.yml"
# Fail when anything differs from the baselines.
- uses: wevm/uphill@main
  with:
    args: --require-baseline
```

### Several Projects

A repository with more than one project runs the action once per project with `working-directory`. Each project keeps its own comment, which the action finds by a marker that holds the directory.

```yaml title=".github/workflows/uphill.yml"
# One comment for the site, and one for the API.
- uses: wevm/uphill@main
  with:
    working-directory: site
- uses: wevm/uphill@main
  with:
    working-directory: api
```

### Pin a Version

The action installs the release that matches its own tag, such as `v0.0.1`. At a branch, such as `main`, it installs the latest release. The `version` input selects another release.

```yaml title=".github/workflows/uphill.yml"
# Install Uphill 0.0.1.
- uses: wevm/uphill@main
  with:
    version: 0.0.1
```

### Local Builds

`version: none` skips the install and runs the `uphill` on `PATH`, such as one the job builds from source.

```yaml title=".github/workflows/uphill.yml"
# Build Uphill with Cargo, then run it.
- run: cargo install --locked --git https://github.com/wevm/uphill uphill
- uses: wevm/uphill@main
  with:
    version: none
```

### Score Pushes

The `compare` input defaults to the base branch on pull requests, and is empty on other events. When it is empty, the action runs plain `uphill` and writes the report to the job summary. On a pull request, `compare: ''` skips the comparison.

```yaml title=".github/workflows/uphill-main.yml"
# Score each push to main, without a comparison or a comment.
name: Uphill
on:
  push:
    branches: [main]

jobs:
  uphill:
    permissions:
      contents: read
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: wevm/uphill@main
```

### Keep a History

`--note` keeps each run on the main branch as Git notes, which trends in `report.html`, `uphill log`, and [readers of `report.json`](../reference/reports.md#report-notes) read. The job fetches the notes, sets a Git identity for them, and pushes them, so it needs `contents: write`. Concurrency keeps two runs from pushing notes at once.

```yaml title=".github/workflows/uphill-main.yml"
# Keep each run on main as a Git note, and push the notes.
name: Uphill
on:
  push:
    branches: [main]
concurrency: uphill-notes

jobs:
  uphill:
    permissions:
      contents: write
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      # The pattern matches refs/notes/uphill and refs/notes/uphill-report.
      - run: |
          git fetch origin 'refs/notes/uphill*:refs/notes/uphill*'
          git config user.name 'github-actions[bot]'
          git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
      - uses: wevm/uphill@main
        with:
          args: --note
      - run: git push origin 'refs/notes/uphill*:refs/notes/uphill*'
```

A pull request's `report.html` draws the same trends when its job clones the full history and fetches the notes before the action runs.

```yaml title=".github/workflows/uphill.yml"
# Give the pull request's report the history of its base branch.
- uses: actions/checkout@v7
  with:
    fetch-depth: 0
- run: git fetch origin refs/notes/uphill:refs/notes/uphill || true
- uses: wevm/uphill@main
```

### Summary Only

`comment: false` keeps the report in the job summary only. A pull request from a fork has a read-only token, so the action cannot comment there. It writes a warning instead of failing the job.

```yaml title=".github/workflows/uphill.yml"
# Write the report to the job summary only.
- uses: wevm/uphill@main
  with:
    comment: false
```

### Keep the Reports

The reports stay in `.uphill/` after the action runs, so a later step can upload them. `.uphill/` is a hidden directory, which the upload step skips unless `include-hidden-files` is set.

```yaml title=".github/workflows/uphill.yml"
# Keep report.html, report.json, and report.md as a workflow artifact, also when Uphill fails.
- uses: wevm/uphill@main
- if: always()
  uses: actions/upload-artifact@v7
  with:
    include-hidden-files: true
    name: uphill
    path: .uphill/
```

## More

- [Action Reference](../reference/action.md): Every input and output.
- [Comparing Revisions](./comparing.md): How timings compare in pairs.
- [Baselines](./baselines.md): Gate on accepted values.
- [Installation](../introduction/installation.md): The install script the action runs.
