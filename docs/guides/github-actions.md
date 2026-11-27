# GitHub Actions

The Uphill action scores each pull request, compares it with its base branch, and keeps one comment on the pull request with the report.

## Overview

The action installs Uphill, runs `uphill --compare` against the base branch, and keeps one comment with a score gauge per category and a link to the HTML report. Each later run updates the same comment. The job fails when `uphill` fails.

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

```yaml title=".agents/uphill/uphill.yaml"
# Install packages in the base branch's worktree too.
setup: bun install --frozen-lockfile
```

### Trust the Packs

Pack scripts run with the job's permissions, so run the workflow on `pull_request`. A `pull_request_target` workflow with secrets runs the packs that a fork pins with those secrets.

```yaml title=".github/workflows/uphill.yml"
# Run with the permissions of the pull request, not of the base repository.
on: pull_request
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

The comment has one row per category, with its score gauge on the base branch and on the pull request. The base branch's scores come from running its scripts once in the comparison. A category that fails shows ✗, and one that the base branch lacks shows –.

The table is `comment.md`, which Uphill writes beside its reports (see [Reports](../reference/reports.md#commentmd)). "View the full report" opens `report.html`, which the action uploads as an unzipped artifact. A failed run adds its exit status, and the end of its output when Uphill wrote no report.

The last line names the command, the commit, and the workflow run. The job summary holds the comment and the whole `report.md`:

```md
<!-- The next line is the marker that the action finds the comment by. -->
<!-- uphill: . -->
## Uphill

<!-- comment.md: one row per category, with a gauge for each revision. -->
<table><tr><th align="left">Category</th><th>main</th><th>This run</th></tr><tr><td><b>Footprint</b></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/100.svg" width="48" height="48" alt="Footprint: 100"></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/100.svg" width="48" height="48" alt="Footprint: 100"></td></tr><tr><td><b>Speed</b></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/86.svg" width="48" height="48" alt="Speed: 86"></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/51.svg" width="48" height="48" alt="Speed: 51"></td></tr></table>

**[View the full report →](https://github.com/acme/slugify/actions/runs/1/artifacts/2)**

> [!CAUTION]
> `uphill --compare origin/main --require-targets` exited with status 1.

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

### Publish to Pages

`publish: true` commits each `report.html` to the `gh-pages` branch and serves it with GitHub Pages, at `uphill/pr-<number>/` for a pull request. The pull request shows "View deployment" with its URL, and the comment links it. The job needs write access to contents, deployments, and Pages.

```yaml title=".github/workflows/uphill.yml"
# Publish each pull request's report, and link it from a deployment.
name: Uphill
on: pull_request

jobs:
  uphill:
    permissions:
      contents: write
      deployments: write
      pages: write
      pull-requests: write
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: wevm/uphill@main
        with:
          publish: true
```

The first run creates the Pages site from `gh-pages`, when the repository has none. A site that GitHub serves from another source stays as it is, and the comment links the artifact instead.

### Show Badges

A published run on a branch also writes one badge per category to `uphill/<branch>/badges/`, in the format of Shields endpoint badges. Score pushes to `main` with `publish: true`, then add each badge to the README.

```md title="README.md"
<!-- The speed score of the last run on main, colored by its band. -->
[![Speed](https://img.shields.io/endpoint?url=https://acme.github.io/slugify/uphill/main/badges/speed.json)](https://acme.github.io/slugify/uphill/main/)
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
