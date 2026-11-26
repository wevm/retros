# Reports

Each run prints a summary and writes three reports to `.uphill/` in the project root: `report.json`, `report.md`, and `report.html`. The reports make no network requests.

## Terminal

The summary lists each category with its score, then each metric with its value and target. A list shows its 3 worst items, a failed check shows its note, and checks that skip collapse into one count, such as `– 41 skipped`. Then come the top 5 opportunities, up to 10 changes since the baselines, and the changes against `--compare`.

```text
$ uphill
▲  42  footprint
  ▲   3  bundle       27.5 kB                   target 5 kB
  ■  57  export-size  p90 9 kB (viem/op-stack)  target 5 kB
                      12 kB  viem/chains
                      9 kB   viem/op-stack
                      8 kB   viem/utils
      –  modules      412
  ■  67  links        2 of 3 pass
                      fail  about.html

■  75  speed  median of 5 rounds
  ▲  33  render      p90 5 ms (parse)  target 2 ms
                     5 ms  parse
                     2 ms  encode
  ●  96  throughput  2588              target 2000, weight 2

Opportunities
  +29  footprint  bundle                     27.5 kB → 5 kB
  +19  speed      render parse               5 ms → 2 ms
  +11  footprint  links about.html           fail → pass
  +3   footprint  export-size viem/chains    12 kB → 5 kB
  +3   footprint  export-size viem/op-stack  9 kB → 5 kB

Since the baseline
  worse  footprint  export-size viem/chains  10 kB → 12 kB
  new    speed      throughput

Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
```

## `report.md`

The Markdown report suits a pull request comment or a job summary. It lists category scores, the top 10 opportunities, and the changes since the baselines and against `--compare`. Each category then has a table of metrics and the 3 worst items of each list.

```md
# Hill climb report

| Category | Score |
| --- | --- |
| Footprint | ▲ 42 |
| Speed | ■ 75 |

## Opportunities

| Gain | Category | Change | Value | Target |
| --- | --- | --- | --- | --- |
| +29 | Footprint | Bundle | 27.5 kB | 5 kB |
| +19 | Speed | Render `parse` | 5 ms | 2 ms |
```

With subjects, the first table has one row per subject and one column per category.

## `report.html`

The HTML report is one file with inline styles and scripts, so it opens from disk or from a CI artifact. Each category has a gauge and a grid of metrics, with each value on a bar against its target. Checks that skip close under a Skipped group after the grid.

```sh
# Open the report on macOS. On Linux, use xdg-open.
open .uphill/report.html
```

The page follows the system's light or dark scheme, fits a phone screen, and respects reduced motion. With subjects, it has one tab per subject and a table that compares them.

Each list expands to its items, worst first, with their targets and notes. A list shows up to 100 items, and `report.json` has the rest.

## Run History

`uphill --note` keeps each run as a Git note on its commit, under `refs/notes/uphill`. The note holds each category's score and each item's value, in the metric line format under one header per category.

```text
$ git notes --ref uphill show HEAD
[footprint 100]
source-size 172B
[speed 50]
slugify-time 1006ns
```

`report.html` charts each category's score and each metric's value across the last 99 noted runs on the first-parent history and the current run. It scores past values with the current configuration.

A metric's chart draws its target as a dashed line when the target is near its values. Each caption states the change since the first run. Hovering a chart, or focusing it and pressing the arrow keys, names each run's commit, date, and value.

## Report Notes

`uphill --note` also keeps the run's `report.json`, byte for byte, as a note on the same commit under `refs/notes/uphill-report`. A dashboard or other tool reads each noted commit's full report from the repository, without a clone or CI artifacts.

On GitHub, a public repository serves a note as a raw file named after its commit. A ref with many notes stores them under fan-out paths, such as `fd/268f9…`, so a reader that lists the ref's tree finds both forms.

```sh
# Read the report noted on HEAD, by its full SHA.
curl "https://raw.githubusercontent.com/wevm/uphill/refs/notes/uphill-report/$(git rev-parse HEAD)"
```

## `dashboard.html`

[`uphill dashboard`](cli.md#uphill-dashboard) writes one page from the noted runs of several repositories. Each subject of each repository is a row, with the category scores of its newest noted run, and a table compares every row.

Each row's section charts every category across the last 99 noted runs. Its report note adds each metric's value, target, and score, and the opportunities. A run noted before report notes existed shows the values of its compact note instead.

```text
# 1. Read each repository's notes.
$ uphill dashboard wevm/uphill ../app
# 2. Open the page, which makes no network requests.
$ open .uphill/dashboard.html
```

## `report.json`

The JSON report has every value in base units: bytes for sizes and nanoseconds for times. Keys keep a fixed order, so the same run writes the same bytes. Fields that do not apply are `null`.

```json
{
  "version": 1,
  "categories": [],
  "opportunities": [],
  "changes": [],
  "comparison": null
}
```

### `version`

- **Type:** number

The version of the JSON format, which is `1`.

### `categories`

- **Type:** array of objects

One object per category and subject, in the order the configuration declares them.

| Field         | Type             | Description                                                                                           |
| ------------- | ---------------- | ----------------------------------------------------------------------------------------------------- |
| `name`        | string           | The category name, such as `footprint`                                                                |
| `title`       | string           | The `title` key, or the name in words                                                                 |
| `subject`     | string or `null` | The subject, such as `node`                                                                           |
| `description` | string or `null` | The `description` key                                                                                 |
| `score`       | number or `null` | The category score, or `null` when it failed or no metric has a score                                 |
| `rounds`      | number           | How many times the script ran, when the category scored                                               |
| `metrics`     | array            | The metrics, in the order the configuration declares them, when the category scored                   |
| `error`       | object           | The `message` and `details` of a failure, such as the end of standard error, when the category failed |

```json
{
  "name": "footprint",
  "title": "Footprint",
  "subject": null,
  "description": null,
  "score": 42,
  "rounds": 1,
  "metrics": []
}
```

### Metric Fields

Each object in `metrics` describes one metric and its items.

| Field         | Type             | Description                                                                          |
| ------------- | ---------------- | ------------------------------------------------------------------------------------ |
| `name`        | string           | The metric name                                                                      |
| `title`       | string           | The `title` key, or the name in words                                                |
| `description` | string or `null` | The `description` key                                                                |
| `kind`        | string           | `size`, `time`, `count`, or `check`                                                  |
| `unit`        | string or `null` | `B` for sizes, `ns` for times, and `null` for counts and checks                      |
| `value`       | number or `null` | The collapsed value, the share of checks that pass, or `null` for a check that skips |
| `item`        | string or `null` | The item the roll-up selected                                                        |
| `score`       | number or `null` | The metric score, or `null` without a target or for a check that skips               |
| `target`      | number or `null` | The target of the selected item, or the total for a `sum`                            |
| `default`     | number or `null` | The default target, which each item without its own target must meet                 |
| `note`        | string or `null` | The note of a metric written without items                                           |
| `better`      | string           | `lower` or `higher`                                                                  |
| `weight`      | number           | The `weight` key                                                                     |
| `rollup`      | string or `null` | `p90`, `p10`, `median`, `max`, `min`, or `sum`, and `null` for checks                |
| `items`       | array            | Each item's `name`, `value`, `target`, and `note`, worst first                       |

An item's `target` is its own target, from the baseline file, a pattern, or its line, and `null` when it equals the metric's `default`. A check item's `value` is `true` or `false`, or `null` when it skips.

```json
{
  "name": "export-size",
  "title": "Export size",
  "description": null,
  "kind": "size",
  "unit": "B",
  "value": 9000,
  "item": "viem/op-stack",
  "score": 57,
  "target": 5000,
  "default": 5000,
  "note": null,
  "better": "lower",
  "weight": 1,
  "rollup": "p90",
  "items": [{ "name": "viem/chains", "value": 12000, "target": null, "note": null }]
}
```

### `opportunities`

- **Type:** array of objects

Every opportunity with a gain of at least half a point, largest first. `items` is how many items the change covers, and `gain` is the category points it adds, rounded.

```json
{
  "category": "footprint",
  "subject": null,
  "metric": "bundle",
  "item": null,
  "items": 1,
  "value": 27500,
  "target": 5000,
  "gain": 29
}
```

### `changes`

- **Type:** array of objects, or `null`

The changes since the baselines, or `null` when no metric has a baseline. `change` is `worse`, `better`, `new`, `gone`, `looser`, or `tighter`. `before` and `after` are `null` for a change without that side.

```json
{
  "category": "footprint",
  "subject": null,
  "metric": "export-size",
  "item": "viem/chains",
  "change": "worse",
  "before": 10000,
  "after": 12000
}
```

### `comparison`

- **Type:** object, or `null`

The result of `--compare`, or `null` without it. `reference` names the revision and its commit. `scores` has each category's score in the revision, with the revision's `error` when its script failed there. `changes` has the same objects as the top-level `changes`. Each timing has `before` and `after` medians, the number of `pairs`, and how many pairs `agree`, and its `change` can be `within noise`.

```json
{
  "reference": "main (ac66366)",
  "scores": [
    { "category": "speed", "subject": null, "score": 100, "error": null }
  ],
  "changes": [],
  "timings": [
    {
      "category": "speed",
      "subject": null,
      "metric": "slugify-time",
      "item": null,
      "change": "worse",
      "before": 440,
      "after": 938.5,
      "pairs": 8,
      "agree": 8
    }
  ]
}
```
