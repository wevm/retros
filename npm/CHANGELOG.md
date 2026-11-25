# uphill

## 0.0.4

### Patch Changes

- 2266bcd: `uphill --update` wrote baselines to `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration, instead of each metric's `baseline` key, so the configuration kept only what people write. Uphill still read `baseline` keys, and the next `uphill --update` moved them into the baseline file.
- e743bb2: Titled `report.md` and `report.html` "Hill climb report".
- 03fa96e: Moved command line parsing to incur. `--help` listed each command's arguments and flags, `uphill <command> --help` described one command, `--version` printed only the version, `--llms` printed a manifest for agents, `uphill completions <shell>` printed a completion script, and `uphill skills add` installed agent skills generated from the commands.
- 543a7fb: Moved targets into `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration, where each item held its accepted value and its target side by side, such as `viem/chains: { baseline: 61kB, target: 80kB }`. A key with `*` and only a target became a pattern, `"*": { target: 5kB }` the default, and `bundle: { target: 5kB }` targeted a metric before its first update. An item's own target in the file took the final say over patterns and line targets. Uphill still read the short form `61kB target 80kB` of older baseline files, and the `target`, `targets`, and `baseline` keys of older configurations, which the next `uphill --update` moved into the baseline file. `uphill add` copied a pack's default targets and patterns into the project's baseline file.
- 543a7fb: Fixed `uphill --update` failing with "could not write uphill.baseline.yaml" when a run recorded nothing and the project had no baseline file yet.
- 0a44ea6: Added TOML to `uphill add`. A YAML pack went into a TOML project as tables and a TOML pack into a YAML project, with the comments above each category and metric. A project without a configuration took the pack's format, and `uphill add --update` merged TOML projects too.
- cfbeb22: Added `uphill add --update [category...]`, which fetched the pack of each added category and merged its changes since the commit the category's comment recorded. A category or script that only one side changed took that side, and one that both changed was a conflict that wrote nothing unless `--force` took the pack's version. Each example gained a `README.md` that `uphill add` printed, saying what to fit.
- 085bf7c: Added `uphill add <pack> [category...]`, which copied a pack's categories into `uphill.yaml` and the scripts their `run` commands named into the project. A pack was any directory with an `uphill.yaml`: a local directory, `owner/name/path` on GitHub such as `wevm/uphill/examples/web-app`, or a Git URL followed by `//path`, each with an optional `@ref`. `--as <name>` added one category under another name.

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
