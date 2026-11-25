# Set up Uphill in this project

Uphill scores a project against its targets the way Lighthouse scores a page. Scripts in the project measure it and append metric lines to the file at `$UPHILL_METRICS`. Uphill runs the scripts, scores each measurement against its target, and lists the changes that would raise each score most. Use the Uphill skill at the end of this brief to create the metrics and their targets.

{example}

## Steps

1. Find the project's surfaces: what its users import, open, call, or run, such as entry points, pages, endpoints, or commands. Then create their metrics and targets with the Uphill skill.
2. Pick two to four categories, such as `footprint`, `speed`, and `quality`. Each category has one script, kept in `.agents/metrics/` beside `.agents/uphill.yaml`. The examples keep both at their root, which works too.
3. Write each script in the project's language, or in TypeScript run by Bun. Each script runs the tools the project already uses and turns what they report into metric lines. Such tools include its bundler, test runner, type checker, linter, formatter, benchmark harness, and Lighthouse. Measure a value by hand only when no tool reports it.
4. Write `.agents/uphill.yaml` with each category's `run` command and its metrics, and give each metric a comment saying what it measures. Write `.agents/uphill.baseline.yaml` with a target the project must meet for each size, time, and count, unless every line sets its own.
5. When one target does not fit every item, have the script write that item's own target on its line. A root entry point that bundles every subpath is one such item. Put the reason in a comment beside the code that sets the target.
6. When a measurement misses, have the script write the tool's reason as its note. A failing test's message and the lint rule that fired are such reasons.
7. Run `uphill check` and fix what it reports until it passes. Then run `uphill` and read the scores and opportunities.
8. Add `.uphill/` to `.gitignore`. When the scores look right, run `uphill --update` to record each value beside its target in `.agents/uphill.baseline.yaml`, and commit both files.

## Configuration

```yaml
# .agents/uphill.yaml. Scripts run from the project root.
footprint:
  run: bun .agents/metrics/footprint.ts
  # Gzipped bytes of each entry point, bundled and minified.
  entry-size: {}

quality:
  run: bun .agents/metrics/quality.ts
  # Whether each test passes.
  tests: {}
  # Percentage of lines tests cover in each source file.
  coverage: { better: higher }
```

```yaml
# .agents/uphill.baseline.yaml. A record with only a target sets a metric's default target, and
# a key with `*` sets targets by item pattern. `uphill --update` adds each value beside its target.
footprint:
  entry-size: { target: 5kB }

quality:
  coverage:
    "*": { target: 90 }
    "src/legacy/*": { target: 60 }
```

- Metric keys: `poor` (the value that scores 50, twice the target by default), `better: higher` when more is better, `varies: true` for a size or count that differs between runs, `weight` (default 1), `score` (how a list collapses: `p90` by default, or `p10` when higher is better, `median`, `max`, `min`, `sum`), `title`, and `description`.
- Category keys: `run`, `timeout` (default `10m`), `title`, and `description`.
- A check always scores the share of items that pass. A size, time, or count scores only when every item has a target, from the baseline file or from its line, and a target is greater than zero.
- A script whose first run writes a time, or a metric marked `varies`, runs 5 times for the median, and `timeout` applies to each run.
- A top-level `setup` command, such as the project's install command, prepares each fresh checkout that `uphill --compare` and `uphill blame` create.

## Metric lines

Each line is `metric [item] value [target <value>] [note <text>]`, appended to the file at `$UPHILL_METRICS`:

```text
install-size 45.6kB
entry-size "." 1.4kB target 2kB
entry-size ./date 408B
call-time weekday 1.2µs
tests "date.test.ts > formats a weekday" fail note "expected 'Mon' to be 'Monday'"
lint src/date.ts fail note "line 8: Variable 'week' is declared but never used."
```

- Values: sizes in `B`, `kB`, `MB`, or `GB` (decimal), times in `ns`, `µs`, `ms`, or `s`, counts without a unit, and checks as `pass` or `fail`, or `skip` when the check does not apply. One metric uses one kind of unit.
- Quote an item or note that contains a space, and escape `"` and `\` inside it with `\`. Quoting any item works.
- A line with an item adds one entry to a list metric. Uphill scores each item against its own target and lists the worst first.
- A script that exits with a non-zero status fails its category. A failing check is a measurement, so a script that wrote one still exits with status 0.
- Standard output and standard error are logs. Uphill shows the end of standard error when a script fails.
- Scripts inherit the environment Uphill runs in, so a script sets any variable its tools' results depend on.

{skill}