# CLI

The `uphill` binary runs a project's scripts, scores the metrics, and writes reports. Its other commands check scripts, brief an agent, add and update packs, read the noted runs of several repositories, and trace a metric through the Git history.

## Usage

`uphill` runs every category, or only the category it names. Flags combine, such as `--compare main --require-targets`.

```text
uphill [category] [--compare <ref>] [--compact] [--note] [--update] [--require-baseline] [--require-targets]
uphill add <pack>[@ref] [category...] [--yes] [--allow-fresh]
uphill update [pack...] [--yes] [--allow-fresh]
uphill remove <pack>
uphill audit
uphill check
uphill dashboard <repository>...
uphill init
uphill log <category> <metric> [item] [--subject <name>]
uphill blame <category> <metric> [item] [--subject <name>] [--since <ref>]
```

```sh
# Score every category.
uphill

# Score only the speed category.
uphill speed
```

## Commands

### `uphill`

Runs each category's script, scores its metrics, lists the opportunities that raise the scores most, and compares sizes, counts, and checks with each metric's baseline. It writes `report.json`, `report.md`, and `report.html` to `.uphill/`.

```sh
# Score every category and write the reports.
uphill
```

### `uphill add`

Pins a pack to a full commit, vendors its files into `.agents/packs/<name>/`, and adds its entry to the configuration's `packs` key. A remote pack is `owner/repo/path` on GitHub or an `https` URL followed by `//path`, with an optional `@ref`. A local pack starts with `./` or `../`.

Names after the pack limit the entry to those categories. Before it writes, `uphill add` prints the pack's files and capabilities and asks for confirmation. `--yes` writes without asking, and a run without a terminal needs it. `--allow-fresh` takes a commit less than a day old.

The [Packs](../guides/packs.md) guide lists each check.

```sh
# Pin the typescript pack to the newest commit of main.
uphill add wevm/uphill/packs/typescript@main
```

### `uphill update`

Moves each remote pack, or each pack it names, to the newest commit of the ref that its entry's comment names, or of the default branch. It refuses a commit that does not descend from the pinned one, and asks for confirmation as `uphill add` does.

```sh
# Review and take the newest commit of each remote pack.
uphill update
```

### `uphill remove`

Removes a pack's entry from the configuration, and the vendored files of a remote pack. It takes the pack's directory name, such as `typescript`.

```sh
# Remove the entry and .agents/packs/typescript/.
uphill remove typescript
```

### `uphill audit`

Fetches each remote pack at its pinned commit and compares it with the vendored files. It exits with status 1 when a vendored file differs from the pinned commit, when either side lacks a file, or when no branch or tag contains the commit.

```text
$ uphill audit
ok       wevm/uphill/packs/typescript at 94bd79a
```

### `uphill check`

Runs each script once and checks the metric lines it writes, without scoring. It reports invalid lines, metrics that the configuration lists but the script does not write, and metrics written but not listed. It writes no reports.

```text
$ uphill check
✓ pages  2 metrics

1 passed
```

### `uphill dashboard`

Reads the newest runs that [`--note`](#--note) kept in each repository, prints their scores, and writes [`dashboard.html`](reports.md#dashboardhtml) to `.uphill/` in the current directory. A repository is a Git URL, a local path, or `owner/name` on GitHub.

Uphill fetches the default branch as commits only, and both notes refs, into a temporary repository with Git's own credentials, so private repositories need no other token. It exits with status 2 when Git cannot fetch a repository.

```text
$ uphill dashboard wevm/uphill
wevm/uphill  214a95c  2026-11-23  report note
  ■  62  footprint
  ●  97  speed

Wrote .uphill/dashboard.html
```

### `uphill init`

Prints a brief that a person gives their agent to set up Uphill in the project. The brief lists the steps, the configuration keys, and the metric line format. It ends with the [`creating-metrics`](../../SKILL.md) skill, which sets out how to choose metrics and ambitious targets.

The brief links the closest example: a Rust API for a Cargo project, a website for a package with pages or a web framework, and a TypeScript library for other packages.

```text
$ uphill init
# Set up Uphill in this project
...
## Steps

1. Find the project's surfaces: what its users import, open, call, or run, such as entry points, pages, endpoints, or commands.
```

### `uphill log`

Lists the commits on the first-parent history that changed a metric's baseline, newest first, with each value and the value before it.

```text
$ uphill log footprint source-size
footprint source-size in the baselines of uphill.baseline.yaml

  fd268f9  2026-11-23  172 B  from 138 B  accept size
  84337c9  2026-11-23  138 B              init
```

When no baseline records the metric, such as a timing, `log` lists the runs that [`--note`](#--note) kept instead.

```text
$ uphill log speed slugify-time
speed slugify-time in the runs noted in refs/notes/uphill

  fd268f9  2026-11-23  1 µs    accept size
  fa8deea  2026-11-23  981 ns  normalize accents
  ac66366  2026-11-23  803 ns  accented input
  84337c9  2026-11-23  646 ns  init
```

### `uphill blame`

Names the commit that last moved a metric. When baselines record the metric, `blame` reads them like `log`. Otherwise it checks out commits in temporary worktrees, measures each one, and bisects for the first commit that measures what `HEAD` measures.

```text
$ uphill blame footprint source-size
fd268f9 moved footprint source-size from 138 B to 172 B, by the baselines in uphill.baseline.yaml
  2026-11-23  accept size
```

A time moved at the first commit that is within noise of `HEAD` in interleaved pairs. Each commit runs the category's `run` command from its own configuration, so a script that moved still runs. The top-level `setup` prepares each checkout.

```text
$ uphill blame speed slugify-time
Bisecting speed slugify-time across 4 commits, measuring each in a temporary checkout
  HEAD     1.1 µs
  84337c9  510.5 ns
  ac66366  476 ns
  fa8deea  992 ns  same as HEAD

fa8deea moved speed slugify-time from 476 ns to 1.1 µs
  2026-11-23  normalize accents
```

## Arguments

### `category`

- **Type:** a category name
- **Default:** every category

With `uphill`, runs only the named category. With `log` and `blame`, names the category of the metric. An unknown name exits with status 2 and lists the categories.

```text
$ uphill speeed
error: unknown category 'speeed'. uphill.yaml declares footprint, speed
```

### `metric`

- **Type:** a metric name

The metric that `log` or `blame` traces. An unknown name exits with status 2 and lists the category's metrics.

```text
$ uphill log footprint nope
error: unknown metric 'nope'. footprint configures source-size
```

### `item`

- **Type:** an item name
- **Default:** none

One item of a list metric, for `log` and `blame`. Quote an item that contains a space.

```sh
# Trace the size of one page.
uphill log pages page-size pricing.html
```

## Flags

### `--compact`

Prints the category scores on one line, then each missed target with its note, the opportunities, and the changes, for agent loops. Uphill still writes every report.

```text
$ uphill --compact
pages 68

Missed targets
  pages  page-size pricing.html  31 kB  target 20 kB
  pages  has-title about.html    fail                 no <title> element

Opportunities
  +17  pages  has-title about.html    fail → pass
  +11  pages  page-size pricing.html  31 kB → 20 kB
```

### `--note`

Keeps this run's category scores and each item's value as a Git note on `HEAD`, under `refs/notes/uphill`. `report.html` draws trends from the notes, and `uphill log` reads them. It also keeps `report.json` under `refs/notes/uphill-report`, for [readers outside the repository](reports.md#report-notes). Uphill refuses with status 2 when tracked files differ from `HEAD`, so a note always describes its commit.

```text
$ uphill --note
...
Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html, a note on fd268f9
```

Git does not push or fetch notes by default. Push them to share them, and fetch them in another clone. Trends and `uphill log` walk the first-parent history, so they need a full clone.

```sh
# Share both notes refs, and fetch them in another clone.
git push origin 'refs/notes/uphill*:refs/notes/uphill*'
git fetch origin 'refs/notes/uphill*:refs/notes/uphill*'
```

### `--subject <name>`

Selects the subject of the metric for `log` and `blame`, when a category's lines have subjects.

```sh
# Trace the startup time of the node subject.
uphill blame speed startup --subject node
```

### `--since <ref>`

Limits the commits that `blame` bisects to those since `<ref>`, and measures `<ref>` as the oldest one. Without it, `blame` bisects every commit on the first-parent history. When `<ref>` measures what `HEAD` measures, `blame` reports that nothing moved the metric since then.

```sh
# Look for the commit that slowed startup since the last release.
uphill blame speed startup --since v1.2.0
```

### `--compare <ref>`

Measures the revision `<ref>` in a temporary Git worktree, and adds the changes against it to every report. Timings run in 8 interleaved pairs. Sizes, counts, and checks compare with the baselines of `<ref>`, or with a run of it when it has none.

```sh
# Compare the current checkout with the main branch.
uphill --compare main
```

### `--update`

Writes this run's sizes, counts, and checks to the baseline file beside the configuration, `uphill.baseline.yaml` or `uphill.baseline.toml`, each beside the target it scored against. It also moves the `target`, `targets`, and `baseline` keys of an older configuration into that file, and keeps the rest of the configuration as it was.

```sh
# Accept this run as the baseline.
uphill --update
```

### `--require-baseline`

Exits with status 1 when anything differs from the baselines, better or worse, or when no metric has a baseline.

```text
$ uphill --require-baseline
...
4 changes since the baseline. Run `uphill --update` to accept them
```

### `--require-targets`

Exits with status 1 when a metric misses its target or a script fails, and lists each miss with its item.

```text
$ uphill --require-targets
...
Missed targets
  speed  slugify-time  917 ns  target 500 ns

1 target missed
```

### `--help`

Prints the usage, the commands, and the flags. `-h` is the short form. After a command, such as `uphill blame --help`, it prints that command's arguments and flags.

```sh
# Print help.
uphill --help
```

### `--version`

Prints the version, such as `0.0.3`. `-V` is the short form.

```sh
# Print the version.
uphill --version
```

### `--llms`

Prints a manifest of the commands in Markdown for an agent. `--llms-full` adds each command's arguments and flags, and `uphill <command> --schema` prints them as JSON Schema.

```sh
# List the commands for an agent.
uphill --llms
```

### `completions`

Prints a completion script for a shell: `bash`, `zsh`, `fish`, or `nushell`.

```sh
# Complete uphill commands in zsh.
eval "$(uphill completions zsh)"
```

### `skills`

Writes agent skills generated from the commands and their help, one per command. `skills add` installs them in `~/.agents/skills`, and `skills list` lists them and shows which are installed.

```sh
# Install the Uphill skills for local agents.
uphill skills add
```

## Exit Status

| Status | Meaning                                                                                              |
| ------ | ---------------------------------------------------------------------------------------------------- |
| 0      | Every category ran and every gate passed                                                             |
| 1      | A category failed, or `--require-baseline` or `--require-targets` failed                             |
| 2      | Uphill could not run, such as for an unknown argument, an invalid configuration, or a failed `setup` |

```sh
# Print the exit status of the last run.
uphill; echo $?
```

## Environment

Uphill sets `UPHILL_METRICS` for each script to the path of a new, empty file. The script appends its metric lines to it, and Uphill reads it after the script exits.

```sh
# Write one metric line from a script.
echo "bundle-size 6.9kB" >> "$UPHILL_METRICS"
```
