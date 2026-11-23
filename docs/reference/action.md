# GitHub Action

`wevm/uphill` is a composite action. It installs Uphill, runs it with `--compare` on pull requests, and writes the report to the job summary and to one comment on the pull request.

## Usage

The job needs `pull-requests: write` for the comment, and `contents: read` to clone the repository.

```yaml title=".github/workflows/uphill.yml"
# Score each pull request and keep one comment with the report.
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
        with:
          args: --require-targets
```

## Steps

The action runs these steps in order:

1. Install the release that `version` selects with the install script, unless `version` is `none`.
2. Fetch `compare` when the clone lacks it, and run `uphill --compare <ref>` with `args` in `working-directory`.
3. Write `report.md` to the job summary. A failed run adds its exit status and the last 60 lines of its output.
4. On a pull request, create or update the comment when `comment` is `true`.
5. Exit with the status of `uphill`.

```yaml title=".github/workflows/uphill.yml"
# A step after the action runs only when Uphill passed, unless it sets if: always().
- uses: wevm/uphill@main
- run: echo "Uphill passed"
```

## Inputs

### `args`

- **Type:** string
- **Default:** `''`

More arguments for `uphill`, separated by spaces, such as a category or a gate.

```yaml
# Score only the speed category, and fail on a missed target.
with:
  args: speed --require-targets
```

### `comment`

- **Type:** `'true'` or `'false'`
- **Default:** `'true'`

Whether to keep one comment on the pull request with the report. The job summary always has the report.

```yaml
# Write the report to the job summary only.
with:
  comment: false
```

### `compare`

- **Type:** string
- **Default:** the base branch on pull requests, and `''` on other events

The branch, tag, or commit to compare with. When the clone lacks it, the action fetches it from `origin`, with depth 1 in a shallow clone. An empty value skips the comparison.

```yaml
# Compare with the last release tag instead of the base branch.
with:
  compare: v1.2.0
```

### `github-token`

- **Type:** string
- **Default:** `${{ github.token }}`

The token that reads and writes the comment. It needs the `pull-requests` write permission. The token of a pull request from a fork is read-only, so the action writes a warning instead of a comment.

```yaml
# Comment with a token from a GitHub App.
with:
  github-token: ${{ steps.app.outputs.token }}
```

### `version`

- **Type:** string
- **Default:** the action's tag, or `latest`

The release to install, such as `0.0.1`, or `latest`. At a release tag, such as `wevm/uphill@v0.0.1`, the default is that release. Otherwise it is the latest release. `none` skips the install and runs the `uphill` on `PATH`.

```yaml
# Run the uphill that an earlier step built.
with:
  version: none
```

### `working-directory`

- **Type:** string
- **Default:** `.`

The project root, which holds `uphill.yaml`, `uphill.toml`, or `.agents/`. The comment marker holds this directory, so each project in a repository keeps its own comment.

```yaml
# Score the project in the site directory.
with:
  working-directory: site
```

## Outputs

### `exit-code`

- **Type:** string

The exit status of `uphill`: `0`, `1`, or `2`.

```yaml
# Give the action step an id, then read its output in a later step.
- id: uphill
  uses: wevm/uphill@main
- run: echo "uphill exited with ${{ steps.uphill.outputs.exit-code }}"
```

### `report`

- **Type:** string

The absolute path of `report.md`.

```yaml
# Print the report in a later step.
- id: uphill
  uses: wevm/uphill@main
- run: cat "${{ steps.uphill.outputs.report }}"
```

## Comment

The comment starts with a hidden marker, `<!-- uphill: <working-directory> -->`, which the action finds on each later run. It then updates that comment instead of adding one. A report longer than a comment allows keeps its start, and the job summary keeps all of it.

```md
<!-- The marker for working-directory ".". -->
<!-- uphill: . -->
# Uphill report
```
