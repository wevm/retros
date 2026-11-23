# Set up Retros in this project

Retros scores a project against its targets the way Lighthouse scores a page. Scripts in the project measure it and append metric lines to the file at `$RETROS_METRICS`. Retros runs the scripts, scores each measurement against its target, and lists the changes that would raise each score most.

{example}

## Steps

1. Find the project's surfaces: what its users import, open, call, or run, such as entry points, pages, endpoints, or commands.
2. Pick two to four categories, such as `footprint`, `speed`, and `quality`. Each category has one script, kept in `.agents/metrics/`.
3. Write each script in the project's language, or in TypeScript run by Bun. Each script runs the tools the project already uses and turns what they report into metric lines. Such tools include its bundler, test runner, type checker, linter, formatter, benchmark harness, and Lighthouse. Measure a value by hand only when no tool reports it.
4. Write `.agents/retros.yaml` with each category's `run` command and its metrics. Give each metric a target the project must meet, and a comment saying what it measures.
5. When one target does not fit every item, have the script write that item's own target on its line. A root entry point that bundles every subpath is one such item. Put the reason in a comment beside the code that sets the target.
6. When a measurement misses, have the script write the tool's reason as its note. A failing test's message and the lint rule that fired are such reasons.
7. Run `retros check` and fix what it reports until it passes. Then run `retros` and read the scores and opportunities.

## Configuration

```yaml
# .agents/retros.yaml. Scripts run from the project root.
footprint:
  run: bun .agents/metrics/footprint.ts
  # Gzipped bytes of each entry point, bundled and minified.
  entry-size: { target: 5kB }

quality:
  run: bun .agents/metrics/quality.ts
  # Whether each test passes.
  tests: {}
  # Percentage of lines tests cover in each source file.
  coverage: { target: 90, better: higher }
```

- Metric keys: `target`, `targets` (targets by item pattern, such as `{ "src/legacy/*": 60 }`), `better: higher` when more is better, `varies: true` for a size or count that differs between runs, `weight` (default 1), `score` (how a list collapses: `p90` by default, or `p10`, `median`, `max`, `min`, `sum`), `title`, and `description`.
- Category keys: `run`, `timeout` (default `10m`), `title`, and `description`.
- A metric without a target, such as a check or a count, shows its value and is not scored.
- A top-level `setup` command, such as `bun install --frozen-lockfile`, prepares the fresh checkout that `retros --compare` creates.

## Metric lines

Each line is `metric [item] value [target <value>] [note <text>]`, appended to the file at `$RETROS_METRICS`:

```text
install-size 45.6kB
entry-size "." 1.4kB target 2kB
entry-size ./date 408B
call-time weekday 1.2µs
tests "date.test.ts > formats a weekday" fail note "expected 'Mon' to be 'Monday'"
lint src/date.ts fail note "line 8: Variable 'week' is declared but never used."
```

- Values: sizes in `B`, `kB`, `MB`, or `GB` (decimal), times in `ns`, `µs`, `ms`, or `s`, counts without a unit, and checks as `pass` or `fail`. One metric uses one kind of unit.
- Quote an item or note that contains a space, and escape `"` and `\` inside it with `\`.
- A line with an item adds one entry to a list metric. Retros scores each item against its own target and lists the worst first.
- A script that exits with a non-zero status fails its category. A failing check is a measurement, so a script that wrote one still exits with status 0.
- Standard output and standard error are logs. Retros shows the end of standard error when a script fails.
