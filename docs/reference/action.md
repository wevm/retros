# GitHub Action

`wevm/uphill` is a composite action. It installs Uphill, runs it with `--compare` on pull requests, and keeps one comment on the pull request with a score gauge per category and a link to the HTML report.

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
3. Upload `report.html` as an unzipped artifact, which opens in the browser. A repository whose artifacts are off has no link, and the job continues.
4. When `publish` is `true`, push the report to `gh-pages`, build the GitHub Pages site, and create a deployment with the report's URL.
5. Write the comment: Uphill's `comment.md`, with its table of score gauges for the base branch and this run, then a link to the published report or the artifact. A failed run adds its exit status, and the last 60 lines of its output when Uphill wrote no report.
6. Write the comment and the whole `report.md` to the job summary.
7. On a pull request, create or update the comment when `comment` is `true`.
8. Exit with the status of `uphill`.

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

The token that reads and writes the comment. It needs the `pull-requests` write permission. With `publish`, it also needs `contents`, `deployments`, and `pages` write. The token of a pull request from a fork is read-only, so the action writes a warning instead of a comment and does not publish.

```yaml
# Comment with a token from a GitHub App.
with:
  github-token: ${{ steps.app.outputs.token }}
```

### `publish`

- **Type:** `'true'` or `'false'`
- **Default:** `'false'`

Whether to publish `report.html` to GitHub Pages. The action commits it to `gh-pages`, one directory per pull request or branch, and requests a Pages build. A deployment with the report's URL adds "View deployment" to the pull request, and the comment links the report.

| Run                                        | Directory on `gh-pages` | Environment           |
| ------------------------------------------ | ----------------------- | --------------------- |
| Pull request 12                            | `uphill/pr-12/`         | `uphill-preview`      |
| Push to `main`                             | `uphill/main/`          | `uphill`              |
| Pull request 12, `working-directory: site` | `uphill/site/pr-12/`    | `uphill-site-preview` |

Each directory holds `index.html`, `report.json`, and [Shields endpoint](https://shields.io/badges/endpoint-badge) badges: the overall score in `badges/score.json` and as an image in `badges/score.svg`, and each category in `badges/<category>.json`, or `badges/<category>-<subject>.json`. A new `gh-pages` branch gets a `.nojekyll` file, and an existing one keeps its other files.

The workflow token may not create a Pages site. So after the first run pushes `gh-pages`, select it under Settings, Pages, once. A token with administration access creates the site itself. A site that publishes another branch or a workflow stays as it is. After every failure, the action warns, and the comment links the artifact.

```yaml
# Publish each report to GitHub Pages with a deployment.
permissions:
  contents: write
  deployments: write
  pages: write
  pull-requests: write
steps:
  - uses: actions/checkout@v7
  - uses: wevm/uphill@main
    with:
      publish: true
```

> [!NOTE]
> GitHub Pages serves a public site, unless the repository is on GitHub Enterprise Cloud and the site is private. A private repository on a free plan has no Pages site.

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

### `url`

- **Type:** string

The GitHub Pages URL of `report.html` when `publish` is `true` and publishing succeeded, such as `https://acme.github.io/slugify/uphill/pr-12/`. It is empty otherwise.

```yaml
# Print the published report's URL in a later step.
- id: uphill
  uses: wevm/uphill@main
  with:
    publish: true
- run: echo "${{ steps.uphill.outputs.url }}"
```

## Comment

The comment starts with a hidden marker, `<!-- uphill: <working-directory> -->`, which the action finds on each later run. It then updates that comment instead of adding one. A report longer than a comment allows keeps its start, and the job summary keeps all of it.

```md
<!-- The marker for working-directory ".". -->
<!-- uphill: . -->
# Hill climb report
```
