# Scoring

Uphill scores each value from 0 to 100 against its target and collapses each list to one score. It averages metric scores into a category score, and category scores into a project score.

## Curve

Each value scores on Lighthouse's log-normal curve with two control points. A value scores 90 at its target. It scores 50 at the metric's [`poor`](./configuration.md#poor), twice the target by default. Smaller values approach 100 and larger values approach 0.

Like Lighthouse, Uphill adds up to half a point to scores above 90 and then rounds down. A score of 89.9 rounds to 89. Gates compare values, not scores, so a value just past its target never passes.

| Value against a 5 kB target  | Score | Band       |
| ---------------------------- | ----- | ---------- |
| 2.5 kB                       | 99    | good       |
| 5 kB, the target             | 90    | good       |
| 7.5 kB                       | 70    | needs work |
| 10 kB, twice the target      | 50    | needs work |
| 20 kB, four times the target | 10    | poor       |
| 27.5 kB                      | 3     | poor       |

```text
  ●  99  a  2.5 kB   target 5 kB
  ●  90  b  5 kB     target 5 kB
  ■  70  c  7.5 kB   target 5 kB
  ■  50  d  10 kB    target 5 kB
  ▲  10  e  20 kB    target 5 kB
  ▲   3  f  27.5 kB  target 5 kB
```

With `poor` set, the curve takes Lighthouse's own control points, so the same value scores the same as in a Lighthouse report.

```yaml title=".agents/uphill/uphill.yaml"
# Lighthouse's mobile Total Blocking Time: 50 at 600 ms, and 90 at the 200 ms target that
# uphill.baseline.yaml sets.
total-blocking-time: { poor: 600ms }
```

```text
  ■  73  total-blocking-time  350 ms  target 200 ms
```

## Bands

Each score falls in a band, which reports mark with a shape so the band reads without color.

| Shape | Band       | Scores                                           |
| ----- | ---------- | ------------------------------------------------ |
| ●     | good       | 90 to 100                                        |
| ■     | needs work | 50 to 89                                         |
| ▲     | poor       | 0 to 49                                          |
| –     | no score   | a metric without a target, or a check that skips |
| ✗     | failed     | a category whose script failed                   |

## Higher Is Better

A metric with `better: higher` scores the reciprocal: half the target scores 50, and twice the target scores 99. Its list collapses to its p10 by default.

```text
  ■  50  h  40       target 80
  ●  99  i  160      target 80
```

## Lists

Items rank by value divided by their own target, so its own target judges each item. The roll-up takes the item at its nearest rank, and reports name it, such as `p90 31 kB (pricing.html)`.

- **`p90`:** The item with at most a tenth of the list ranked above it. This is the default.
- **`p10`:** The item with at most a tenth of the list ranked below it. This is the default when higher is better.
- **`median`:** The upper middle item.
- **`max`** and **`min`:** The highest and the lowest item.
- **`sum`:** The total of the values against the total of the targets.

A list of checks scores the share of items that pass, so 2 of 3 passing scores 67. Items that `skip` count toward neither side.

```text
■  68  page-size  p90 31 kB (pricing.html)  target 20 kB
■  67  has-title  2 of 3 pass
```

## Categories

A category score is the weighted mean of its metric scores, rounded. Like Lighthouse, it adds the scores as fractions of 1 in the order `uphill.yaml` declares them. A mean of exactly half a point then rounds as Lighthouse rounds it.

A metric with weight 0 has a score but stays out of the mean.

A metric without a target, and a check that skips, have no score and stay out of the mean. A category has no score when its script fails or does not write a metric that the category lists. No score is different from a score of 0.

```text
▲  42  footprint
  ▲   3  bundle       27.5 kB                   target 5 kB
  ■  57  export-size  p90 9 kB (viem/op-stack)  target 5 kB
      –  modules      412
  ■  67  links        2 of 3 pass
```

## Project Score

The project score is the mean of the category scores, rounded the same way. Each subject of a category counts once, and every category carries the same weight. A category without a score stays out of the mean, and a project where no category scores has no project score.

`report.json` holds it as the top-level `score`. The other reports show only the categories, so a poor category stays visible.

```json
{
  "version": 1,
  "score": 59,
  "categories": []
}
```

## Rounds

When a script's first run writes a time, or a metric marked `varies`, Uphill runs the script 5 times. It scores the median of each timing and each varying item. Notes and the other sizes, counts, and checks come from the first run.

```text
■  75  speed  median of 5 rounds
```

## Opportunities

An opportunity is a change and the category points it adds, if one item, one metric, or a whole list met its target. The list leaves out gains under half a point. The terminal shows the top 5, `report.md` shows the top 10, and `report.json` lists them all.

```text
Opportunities
  +29  footprint  bundle                     27.5 kB → 5 kB
  +19  speed      render parse               5 ms → 2 ms
  +11  footprint  links about.html           fail → pass
```
