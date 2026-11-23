# Implementation plan

Retros is continuous metrics and hill climbing. A project's scripts measure it, and Retros scores each measurement against the target that fits it, remembers the accepted values, names the next step, and gates changes that slide back. This plan tracks Retros from foundations to release. Each phase ends with an acceptance gate, and a phase is complete only when its gate passes.

## The Loop

Hill climbing is a loop, and every part of Retros serves one step of it. A person or an agent runs the loop on each change.

| Step | Question | How Retros answers | Status |
| --- | --- | --- | --- |
| Measure | What is the project like now? | Scripts write metric lines. Retros runs them and repeats timings. | Done |
| Score | How far is it from its targets? | Each item scores on Lighthouse's curve against its own target. Metrics roll up into categories. | Done |
| Compare | What did this change move? | `retros.lock` holds the accepted sizes, counts, and checks. Timings compare in interleaved pairs on one machine. | Lock done, timings in Phase 6 |
| Next step | What should change next? | Opportunities rank changes by the category points they add. Notes say why an item misses. | Opportunities done, notes in Phase 7 |
| Gate | Can this change land? | `--require-baseline` fails on any change since the lock, and `--require-targets` fails on a missed target. | Baseline done, targets in Phase 6 |
| Remember | Where has the project been, and what moved it? | `retros.lock` in Git, a history of runs, and metric blame. | Lock done, history in Phase 9 |

## How Retros Maps to Lighthouse

Retros keeps Lighthouse's scoring and depth, and replaces its fixed browser gatherers with the project's own scripts.

| Lighthouse | Retros |
| --- | --- |
| Category, such as Performance or Accessibility | A top-level key in `retros.yaml`, named by the project |
| Audit, such as Largest Contentful Paint or image alt text | A metric |
| Audit details, such as each image without alt text | The metric's items, worst first |
| The p10 and median control points of an audit | The target and twice the target |
| Opportunities with estimated savings | Opportunities with the category points they add |
| Audit descriptions and help text | Notes on metric lines (Phase 7) |
| Passed audits | Metrics at or better than their targets |
| Lighthouse CI assertions | `--require-targets` and `--require-baseline` |
| Chrome traces and DevTools protocol gatherers | Project scripts in any language, which can run Lighthouse itself |

## Goals

| Goal | Target | Evidence |
| --- | --- | --- |
| Any language | A project in any language takes part through scripts and one text file. | Shell, TypeScript, and Rust examples, viem, and zepto |
| Lighthouse scores | The same value and target produce the same score as Lighthouse. | Curve tests against Lighthouse's `shared/statistics.js` and zepto's `bench/metrics.zig` (done) |
| Lighthouse detail | Every metric shows which items miss, by how much, why, and what to change first. | Example and viem reports in Phases 4, 5, and 7 |
| Contextual targets | Each item is scored against the target that fits it, not one number for every item. | Examples and viem |
| Honest comparisons | Exact changes always show. Timing changes show only when repeated runs agree. | Lock tests (done) and the Phase 6 gate |
| Agents climb | An agent runs the loop unattended: it reads the next step, changes code, and confirms the climb. | Phase 8 gate |
| Replace zepto's lighthouse | zepto's report comes from Retros with the same scores within noise. | Phase 10 gate |
| One binary | A single static binary with no runtime to install. | Release assets in Phase 11 |

## Decisions

1. **Name.** Retros. Repository `wevm/retros`, binary `retros`, configuration `retros.yaml`, accepted values in `retros.lock`, and reports under `.retros/`. The names `retros` on npm and crates.io were free on 2026-11-22.
2. **Scripts measure with existing tools, Retros scores.** Scripts run the tools a project already uses, such as Lighthouse, test runners, type checkers, linters, formatters, and benchmark harnesses, and turn their output into metric lines. A script measures a value itself only when no tool reports it. Retros has no built-in measures until the same script recurs across projects, and scripts remain the escape hatch after that. Decided 2026-11-22, with the preference for existing tools added the same day.
3. **YAML configuration.** `retros.yaml` uses YAML, the format most CI and agent configuration already uses. yaml-rust2's event parser keeps each node's position, so errors keep their line and column, and it adds four crates. `retros.lock` stays TOML because it is written by Retros, not by hand. Decided 2026-11-23, replacing TOML.
4. **Metrics go to a file.** Scripts append lines to the file at `$RETROS_METRICS`, not to standard output. Tools that scripts call print to standard output, and a standard output protocol lets any tool inject a metric. GitHub Actions moved from standard output commands to `$GITHUB_OUTPUT` for the same reason. Decided 2026-11-22.
5. **One line per measurement.** The format is `metric [item] value [target <value>]`. An item with a space is quoted.
6. **One target, scored like Lighthouse.** A target is a requirement the project must meet. A value at its target scores 90, twice the target scores 50, and four times the target scores 10. When higher is better, half the target scores 50. Lighthouse's p10 control point becomes the target and its median becomes twice the target. There is no overall score. Decided 2026-11-22, replacing the `target` and `limit` pair.
7. **Targets are contextual.** A target in `retros.yaml` is the metric's default. A script that knows an item needs a different requirement writes the target on the line: a root entry point that bundles the whole library gets a larger target than a subpath, and a health endpoint gets a smaller one than a search endpoint. Each item is scored against its own target, and a list ranks its items by how far past their own targets they are, so a large item that meets a large target never hides a small item that misses a small one. When the project disagrees with a script, `targets` in `retros.yaml` sets targets by item pattern, such as `"viem/chains/*" = "1kB"`, and has the final say. A list where some items have a target and others do not, with no default, is an error. Decided 2026-11-22, with patterns added the same day.
8. **Exact metrics and line targets are locked, timings are compared.** `retros.lock` records sizes, counts, and checks, which are the same on every machine, and the targets scripts set on their lines, so the lock is committed and reviewed like code. Accepting a regression or loosening a script's target is a visible change to `retros.lock` in review. Timings differ between machines, so their values are never locked and compare only within one run on one machine. Decided 2026-11-22.
9. **Opportunities name the next step.** Retros lists the changes that would raise each category's score most, in category points: one item meeting its target, one metric meeting its target, or a whole list when no single item moves the score. Gains under half a point are left out. Decided 2026-11-22.
10. **Gates compare values, and improvements lock in.** `--require-baseline` exits with status 1 when anything differs from `retros.lock`, better or worse, or when there is no lock, like a snapshot test. The lock on `main` then always matches the code, every improvement becomes the new floor, and `--compare` can trust a revision's lock. `--require-targets` exits with status 1 when a metric misses its target or has no value because its script failed. Both compare values, not rounded scores, so a value just past its target never passes as 90.
11. **Retros owns repetition.** When a script's first run writes a time, Retros runs it 5 times for a score, and Phase 6 runs 8 interleaved pairs for a comparison. No project tunes noise by hand. Decided 2026-11-22, removing the `rounds` key.
12. **TypeScript on Bun, no Python.** Scripts in this repository, including the dogfood scripts and the examples, are shell or TypeScript run by Bun. Projects that use Retros pick any language. Decided 2026-11-22.
13. **Pure engine, thin command line.** `crates/engine` is deterministic and has no I/O. `crates/cli` runs scripts, reads and writes files, and drives Git.
14. **Built like Ondine and zyzz.** AGENTS.md, lints, toolchain pin, and testing rules follow those repositories. Work lands directly on `main`.
15. **Subjects.** A line may start with `subject:`, such as `zc: startup 0.64ms`, for projects that measure several harnesses, packages, or builds in one script. Each subject is scored on its own, reports compare subjects side by side, and the lock keys a subject's category as `"zc: speed"`. A category may not mix lines with and without a subject, and a subject that does not write a metric has no score for it. Decided 2026-11-23 for zepto.
16. **Shared runs.** A top-level `run` and `timeout` apply to every category that sets none, and categories with the same `run` share one execution. Each line goes to the category that lists its metric, so one metric in two categories that share a run is an error. Decided 2026-11-23, so zepto's one runner feeds five categories.
17. **`varies`.** A size or count marked `varies = true`, such as peak memory, takes its median across rounds like a time and stays out of the lock, except for its line target. Decided 2026-11-23.
18. **Descriptions.** Categories and metrics take a `description`, and every report shows it, so a reader knows what a number measures without opening `retros.yaml`. Decided 2026-11-23.
19. **`.agents/retros.yaml`.** Retros reads `retros.yaml` or `.agents/retros.yaml` from the current directory or the nearest parent, and it is an error for one directory to have both. Scripts run from the project root, the directory that holds `retros.yaml` or `.agents`, so a configuration kept with a project's agent files still measures the whole project. `retros.lock` sits beside `retros.yaml`, and reports go to `.retros/` in the project root. Decided 2026-11-23.

## The Contract

A project adds `retros.yaml`, at its root or in `.agents/`. Each top-level key is a category, `run` is the script that measures it, and every other key is a metric the script writes. A metric with a target gets a score, and a metric without one shows its value and change only.

```yaml
# Two categories, each measured by its own script.
footprint:
  run: bun metrics/footprint.ts
  # Gzipped bytes of each export. The script sets its own target for exports that need one, and
  # `targets` overrides any item by pattern.
  export-size: { target: 5kB, targets: { "viem/chains/*": 1kB } }
  install-size: { target: 10MB }

speed:
  run: bun metrics/speed.ts
  import-time: { target: 20ms }
```

The script appends lines to the file at `$RETROS_METRICS`, each `[subject:] metric [item] value [target <value>]`. The values below are viem's from the earlier applied metrics trial, and the line target is an example:

```text
install-size 45.6MB
export-size viem:createClient 1.1kB
export-size viem/actions:readContract 31.7kB target 40kB
bench-time "ABI Decode (500 results)" 1.82ms
```

- **Values and units:** `B`, `kB`, `MB`, and `GB` for sizes (decimal), `ns`, `µs`, `us`, `ms`, and `s` for time, no unit for counts, and `pass` or `fail` for checks. One metric uses one kind of unit.
- **Items:** a line with an item adds one entry to a list metric. Retros scores each entry, collapses the list to one score, and lists the entries worst first.
- **Targets:** `target <value>` at the end of a line sets that item's target, in the metric's kind of unit. It overrides the default `target` in `retros.yaml`, and the first matching pattern in `targets` overrides it. In a pattern, `*` matches any run of characters. A target of 0 is an error.
- **Logs:** standard output and standard error are logs. Retros keeps the end of standard error for a failed run.
- **Failure:** a script that exits with a non-zero status gives its category no score.
- **Runs:** each script runs through `sh -c` from the project root, the directory that holds `retros.yaml` or `.agents`, in its own process group. Retros kills the group when the script exits or times out, so background processes do not outlive the run.
- **Subjects:** a line that starts with `subject:` belongs to that subject, and each subject is scored on its own.
- **Optional metric keys:** `targets` (item patterns and their targets), `weight` (default 1), `score` (how a list collapses: `p90` by default, or `p10`, `median`, `max`, `min`, `sum`), `better = "higher"` for metrics where more is better, `varies = true` for a size or count that differs between runs, and `description`. A `better = "higher"` list collapses to its p10 by default.
- **Optional category keys:** `timeout` (default `10m`) and `description`. A top-level `run` and `timeout` apply to every category that sets none, and categories with the same `run` share one execution.

The command line:

```text
retros [category] [--update] [--require-baseline]   Score, list opportunities, and compare with retros.lock
retros check                                        Run each script once and check its lines
```

- **Files:** `retros.lock` is committed. A recorded line target follows its value, as in `"viem/chains" = "61000B target 80000B"`. `.retros/report.html`, `report.json`, and `report.md` are written on every run and ignored by Git.
- **Exit status:** 0 when every category passes, 1 when a category fails or a gate fails, and 2 when Retros cannot run, such as for an unknown argument or an invalid `retros.yaml`.

## Scoring

1. **Repeat timings.** When a script's first run writes a time or a metric marked `varies`, Retros runs it 5 times and takes the median of each timing item and each varying item. Other sizes, counts, and checks come from the first run.
2. **Score each measurement.** Lighthouse's `getLogNormalScore`, ported with its `erf` approximation, scores each value against its own target. Smaller values approach 100 and larger values approach 0. A `better = "higher"` metric scores the reciprocal.
3. **Collapse lists.** Items rank by how far past their own target they are: value divided by target, or target divided by value when higher is better. The roll-up takes the item at its nearest rank, so the p90 is the item with at most a tenth of the list further past its target, and the median is the upper middle. The report names the selected item, such as `p90 9 kB (viem/op-stack)`. A `sum` scores the total of the values against the total of the targets. A check scores the share of items that pass.
4. **Average categories.** Like Lighthouse, a metric's score is rounded to a whole number, and a category is the weighted mean of those scores, rounded. Metrics without a target are shown but not scored, a metric with weight 0 is scored but left out of the mean, and a failed metric drops out instead of counting as zero.

| Value against a 5 kB target | Score | Band |
| --- | --- | --- |
| 2.5 kB | 99 | good |
| 5 kB (the target) | 90 | good |
| 7.5 kB | 70 | needs work |
| 10 kB (twice the target) | 50 | needs work |
| 20 kB (four times the target) | 10 | poor |
| 27.5 kB (viem's p90 export) | 3 | poor |

Changes are reported separately from scores:

- **Exact units:** sizes, counts, and checks compare with `retros.lock` on every run, item by item, labeled worse, better, new, or gone.
- **Timings:** `--compare <ref>` runs 8 interleaved base and head pairs. Retros reports a change when the medians differ by at least 5% and at least 7 of 8 pairs agree on its direction. Otherwise the change reads "within noise". The rule comes from an earlier zepto trial on one machine, so Phase 6 checks it on CI runners.
- **Items:** per-item changes always show, so one regressed export is never hidden by the collapsed value.

## Phase 0: Foundations

Set up the repository, conventions, and checks that every later phase builds on.

- [x] Cargo workspace with `crates/engine` (pure library) and `crates/cli` (the `retros` binary).
- [x] Rust 1.97.0 pinned in `rust-toolchain.toml`, `rustfmt.toml`, and workspace lints from Ondine.
- [x] `clippy.toml` purity bans for the engine, confirmed by a probe: `std::fs` and `HashMap` fail Clippy in `crates/engine` and pass in `crates/cli`.
- [x] AGENTS.md adapted from Ondine and zyzz, including the writing, documentation, and prose conventions, with `CLAUDE.md` linked to it.
- [x] README, MIT LICENSE, `.gitignore`, and this plan.
- [x] Integration test harness: `insta` inline snapshots and binary tests for `--version`, `--help`, and argument errors.
- [x] Verify workflow (format, Clippy, tests) on pushes to `main` and on pull requests.
- [x] `wevm/retros` on GitHub with the first commit on `main`.
- [ ] `.agents/setup` that installs the pinned toolchain and Bun in Amp orbs.

**Gate:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass locally and in CI on `main`.

## Phase 1: Contract and Runner

Read the configuration, run each script once, and read what it wrote.

- [x] `engine::config`: typed `retros.yaml`, unknown keys rejected, defaults applied, errors with line and column.
- [x] `engine::lines`: the metric line parser, with items, quoting, units, and checks.
- [x] `cli`: run each `run` command through `sh -c` in its own process group, from the directory that holds `retros.yaml`, with a fresh `$RETROS_METRICS` file and a timeout that kills the group.
- [x] `retros check`: run each script once and confirm that every configured metric was written with one kind of unit. List metrics written but not configured.
- [x] Fixture projects under `crates/cli/tests/fixtures/`: passing, failing, malformed line, missing metric, and timeout.

**Gate:** integration tests cover each fixture. Every error message is snapshotted and reviewed, and names its file, line, and column.

## Phase 2: Scoring and Reports

Turn metric lines into scores and write the first reports.

- [x] Rounds and medians for timing metrics.
- [x] List roll-ups: `p90`, `p10`, `median`, `max`, `min`, and `sum`.
- [x] The scoring curve, checks, `better = "higher"`, weights, and categories.
- [x] `retros` runs every category and `retros <category>` runs one.
- [x] Terminal summary, `.retros/report.json`, and `.retros/report.md`.

**Gate:** curve tests match values computed by Lighthouse's `shared/statistics.js` at `896399bc` and zepto's `bench/metrics.zig` at `c8a97f7` for the same inputs. Report snapshots for a fixture project are reviewed.

## Phase 3: Baselines, Opportunities, and Contextual Targets

Remember accepted values, name the next step, and score each item against the target that fits it.

- [x] Per-line targets (`target <value>`), with errors for a zero target, a target in the wrong kind of unit, and a list that mixes lines with and without targets when the metric has no default.
- [x] Lists rank items by value against their own target, and reports name the selected item.
- [x] `retros.lock`: sizes, counts, and checks sorted by name, written by `retros --update`, with no timings, so the same code writes the same file on every machine.
- [x] Changes against the lock, labeled worse, better, new, or gone, in the terminal summary, `report.json`, and `report.md`.
- [x] `--require-baseline` exits with status 1 when anything differs from the lock, better or worse, or when there is no lock.
- [x] The lock records the targets scripts set on their lines, including a timing's, and reports a changed one as looser or tighter.
- [x] `targets` in `retros.yaml`: item patterns that have the final say over line targets, with errors for a target in the wrong kind of unit and warnings for a pattern that matches no item.
- [x] The `rounds` key is gone, and timings always run 5 times.
- [x] Opportunities: the top 5 in the terminal and the top 10 in `report.md`, each with the category points it adds.

**Gate:** met. Engine tests cover line targets, patterns overriding them, ranking each item against its own target, mixed targets, and opportunity gains. CLI tests cover `--update`, the lock with line targets, an improvement alone failing `--require-baseline` until it is accepted, a planted regression and loosened target, a missing lock, and an invalid lock.

## Phase 4: Examples and README

Show what a rich, Lighthouse-level setup looks like for common kinds of projects, and describe Retros plainly.

- [x] Ban Python in AGENTS.md: scripts are shell or TypeScript on Bun. The dogfood `metrics/speed.py` is now `metrics/speed.ts`, and the verify workflow installs Bun 1.4.4.
- [x] Examples run real tools instead of hand-rolled measures: Lighthouse for the web app, TypeScript, oxlint, oxfmt, mitata, and Bun's test runner for the library, and clippy, rustfmt, and `cargo test` for the API.
- [x] `examples/typescript-library`: footprint (install size, dependencies, entry size, export size, bytes tree-shaking fails to drop, and an unscored export count the lock tracks), speed (import time per entry point and mitata's median call time per function), and quality (tests, coverage per file, and type errors, lint, and formatting per file, and resolvable entry points). The root entry's target is the sum of its subpaths and 10% more, and formatters that call `Intl` get a larger call-time target by pattern.
- [x] `examples/web-app`: Lighthouse 13 in its default mobile mode on each page. Performance writes Lighthouse's five metrics and total byte weight per page. Accessibility, best practices, and SEO write each audit that counts toward Lighthouse's score as a check per page. The landing page gets larger paint and byte-weight targets by pattern.
- [x] `examples/rust-api`: footprint (binary size and dependencies), latency (median and 95th percentile per endpoint, and throughput), reliability (status codes, a malformed request, surviving a panic, and the error rate under load), and quality (clippy per file, an unscored count of panic sites from clippy's restriction lints, rustfmt per file, and each test). Each endpoint's target comes from the work it does, and its tail may take three times as long. Latency stays a fetch loop because autocannon under Bun reported inconsistent latencies and connection errors.
- [x] Each example misses a few targets on purpose, so its report shows misses, opportunities, and contextual targets.
- [x] README: the tagline "Continuous metrics and hill climbing", a Features section in the style of zyzz's README, "any project" under Philosophy, target-only scoring, targets, baselines, and usage.

**Gate:** CI runs `retros check` in every example through the CLI tests. Every miss in each example's report names its item, its value, and its target. Waiting on review of the README and the example reports.

## Phase 5: viem

Run Retros on a real TypeScript library with thousands of items.

- [ ] `metrics/footprint.ts`: export size for every export (esbuild without code splitting, gzip), with targets by kind of export, such as chains, actions, and clients, and install size.
- [ ] `metrics/speed.ts`: import time for each of the 26 entry points.
- [ ] `metrics/types.ts`, if viem's development install works in an orb. Its postinstall builds contracts with Foundry.
- [ ] `retros.yaml` and `retros.lock` in a local viem checkout. Nothing is committed to viem without approval.

**Gate:** with viem 2.58.4 and esbuild 0.28.2, the export-size list has 1,912 items, matching the applied metrics trial. A planted regression in one module shows as worse in the lock comparison, and nothing else changes. The top opportunities name exports a viem maintainer agrees are worth shrinking.

## Phase 6: Timing Comparisons and Target Gates

Compare timings honestly between two revisions, and fail CI on missed targets.

- [ ] `retros --compare <ref>` checks out `<ref>` in a temporary Git worktree. Timing metrics run 8 interleaved pairs with the verdict rule above. Exact metrics compare with `<ref>`'s `retros.lock` read from Git, and `<ref>` is measured only when it has no lock.
- [ ] A warning when `retros.yaml` or a script it runs differs between the two revisions.
- [ ] The worktree is removed after success or failure.
- [ ] Timing changes appear in the terminal summary, `report.json`, and `report.md`.
- [ ] `--require-targets` exits with status 1 when a metric misses its target or has no value, and lists each miss with its item. It works with and without `--compare`.

**Gate:** an unchanged revision reports no changes. A planted slowdown is reported, and a noisy timing fixture reads "within noise", on both a local machine and a GitHub Actions runner. A failing script leaves no worktree behind. With `--require-targets`, a fixture that misses one target exits with status 1 and names that metric and item, and a fixture that meets every target exits with status 0.

## Phase 7: Details and HTML Report

Explain each miss, and write the report people open.

- [ ] Notes: a line can say why an item misses, such as `alt-text /pricing fail note "hero.svg has no alt text"`. Notes appear under their items in every report. The syntax is decided at the start of this phase.
- [x] Metric descriptions in reports, from the `description` key (decision 18).
- [x] One self-contained `.retros/report.html`: a comparison table and one tab per subject, a gauge per category, score bands, metric rows with descriptions, items worst first with their targets, opportunities, and changes. Notes join the items once they exist.
- [x] Light and dark schemes, phone layout, keyboard access to the tabs, and reduced motion.

**Gate:** the web app example's HTML report reads like a Lighthouse report: every miss names its item, its value, its target, and why. Screenshots at desktop and phone widths, in light and dark schemes, are reviewed. The browser records no network requests.

## Phase 8: Agents

Let the agent a person already works with set up Retros and climb.

- [ ] `retros init` prints a brief for the user's agent: find the project's surfaces (entry points, pages, endpoints, or commands), write one script per category in the project's language or TypeScript on Bun that runs the tools the project already uses, set a target per item where context differs and record the reason beside it, and run `retros check` until it passes. The brief points the agent at the closest example from Phase 4.
- [ ] A compact report for agent loops: scores, misses, and the top opportunities in a few lines. Whether this is a flag or `report.md` is decided at the start of this phase.
- [ ] A skill that runs the loop: score, take the top opportunity, change the code, score again, and keep the change only when the category rose and nothing got worse.

**Gate:** an agent given only the brief sets up a project it has not seen until `retros check` passes. Given an example's report, an agent raises a category score without making any metric worse.

## Phase 9: History and Blame

Remember where the project has been and what moved it.

- [ ] Each run on `main` keeps its report. Where reports live, such as Git notes, a branch, or CI artifacts, is decided at the start of this phase. zepto publishes to GitHub Pages today.
- [ ] `retros log <metric> [item]` prints a metric's values across commits. Exact metrics come from the Git history of `retros.lock`, so they need no stored runs.
- [ ] `retros blame <metric> [item]` names the commit that moved a metric: from lock history for exact metrics, and by measuring and bisecting for timings.
- [ ] Trends in the HTML report.

**Gate:** on Retros' own history, `retros blame footprint dependencies` names the commit that added a dependency, and `retros log` shows the change.

## Phase 10: zepto

Replace zepto's harness metrics report with Retros.

- [x] Subjects, shared runs, and `varies` (decisions 15 to 17). The HTML report shows one tab per subject.
- [x] `zig build metrics` writes metric lines for each harness. zc-mock, the harness commands, and the crash and resume scenario stay in zepto, and its `retros.yaml` holds the targets.
- [x] zepto's CI runs `retros` and publishes `report.html` to GitHub Pages per run.
- [ ] Pages history through Phase 9.

**Gate:** Retros scores zc, the Zig SDK, and the TypeScript package from one run of `zig build metrics` per round, and zepto removes its scoring and report code. Scores move because each metric has one target instead of zepto's `good` and `median` pair, which jxom accepted on 2026-11-23. On one orb, `retros` took 48 s and scored the TypeScript package's speed 85, from 91, since its first request misses a 20 ms target.

## Phase 11: Distribution

Make Retros easy to install and run in CI.

- [ ] Release binaries for Linux (x64 and arm64, glibc and musl) and macOS (x64 and arm64) on GitHub Releases.
- [ ] An install script and an npm package with platform packages, so `npx retros` works.
- [ ] A GitHub Action that runs `retros --compare` on pull requests and keeps one updating comment.
- [ ] The documentation site, following the Writing Documentation conventions.

**Gate:** clean Linux and macOS machines install and run Retros. The Action posts and updates a comment on a test pull request.

## Later

These are not scheduled: built-in measures, such as `retros time` and `retros size` helpers that write metric lines, once the same script recurs across projects; re-measuring only items whose inputs changed; peak memory; and Windows.

## Risks

- **Scripts can measure wrong.** An esbuild code-splitting mistake inflated small viem exports by up to 20×. Earliest signal: a value far from what an existing tool reports for the same thing. Fix: scripts run established tools instead of hand-rolled measures, examples plant controls, and built-in measures come later.
- **Contextual targets can be loosened to pass.** Earliest signal: a pull request that raises a target in a script. Fix: done. `retros.lock` records line targets, so a loosened one is a reviewed change.
- **List metrics are slow.** Export size for 1,912 viem exports took 174 s in an orb. Earliest signal: a viem run over 10 minutes. Fix: compare exact metrics with the lock instead of measuring `<ref>`, then re-measure only changed items.
- **Timings are noisy on shared runners.** Earliest signal: most timing comparisons read "within noise". Fix: more pairs, or a dedicated runner.
- **Targets are guesses.** Earliest signal: every item lands in the same band. Fix: the agent brief asks for a reason beside each target, and reviews challenge targets like code.
