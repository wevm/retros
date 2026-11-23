# uphill

## 0.0.3

### Patch Changes

- 7eb935a: `uphill dashboard <repository>...` reads the newest noted runs of several repositories and writes `.uphill/dashboard.html`: a table of every repository's category scores, each category's trend, and the metrics, targets, and opportunities of each report note. A repository is a Git URL, a local path, or `owner/name` on GitHub, and Git's own credentials read private ones.
- 4068932: Scored like Lighthouse, exactly. A metric's `poor` key sets the value that scores 50, Lighthouse's median control point, which defaults to twice the target. A check can be `skip` when it does not apply, which gives it no score, as Lighthouse gives a not applicable audit no weight. Curve scores now follow `Util.computeLogNormalScore`, which rounds down after a small boost above 90, and category means add scores as Lighthouse does, so some scores read one point lower.
- 401c58e: Added `default` to each metric in `report.json`: the metric's `target` key, which each item without its own target must meet.
- 214a95c: `uphill --note` also keeps the run's `report.json` as a Git note on the same commit, under `refs/notes/uphill-report`, so a dashboard reads each noted commit's full report from the repository. CI that pushes notes now fetches and pushes both refs with `'refs/notes/uphill*:refs/notes/uphill*'`.

## 0.0.2

### Patch Changes

- 1281f68: Fixed an opportunity that groups a list's failing checks, which showed `pass → pass` and now shows `fail → pass`.

## 0.0.1

### Patch Changes

- d0d9be0: Renamed Retros to Uphill. The binary and npm package are `uphill`, the configuration is `uphill.yaml` or `uphill.toml`, reports go to `.uphill/`, scripts write to `$UPHILL_METRICS`, notes live under `refs/notes/uphill`, and the action keeps its comment under an `uphill` marker.
