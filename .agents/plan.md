# Implementation plan

This plan tracks Retros from foundations to release. Retros is continuous metrics and hill climbing. Each metric has one `target`, a requirement the project must meet. A project's scripts measure it, and Retros scores each metric against its target, compares revisions, and writes reports, so every change shows whether the project climbed toward its targets or slid back. Each phase ends with an acceptance gate, and a phase is complete only when its gate passes.

## Goals

| Goal | Target | Evidence |
| --- | --- | --- |
| Any language | A project in any language takes part through scripts and one text file. | Shell fixture projects, viem (TypeScript), and zepto (Zig) |
| Lighthouse scores | The same value and target produce the same score as Lighthouse. | Score tests against values from Lighthouse's JavaScript and zepto's `bench/lighthouse.zig` |
| Honest comparisons | Exact changes always show. Timing changes show only when repeated runs agree. | Comparison tests with planted regressions and unchanged revisions |
| Replace zepto's lighthouse | zepto's report comes from Retros with the same scores within noise. | Phase 6 gate |
| One binary | A single static binary with no runtime to install. | Release assets in Phase 7 |

## Decisions

1. **Name.** Retros. Repository `wevm/retros`, binary `retros`, configuration `retros.toml`, reports under `.retros/`. The names `retros` on npm and crates.io were free on 2026-11-22.
2. **Scripts measure, Retros scores.** Retros has no built-in measures until the same script recurs across projects. Scripts remain the escape hatch after that.
3. **TOML configuration.** Rust's TOML support is solid, TOML has no indentation traps, and Cargo and Python users already know it.
4. **Metrics go to a file.** Scripts append lines to the file at `$RETROS_METRICS`, not to standard output. Tools that scripts call print to standard output, and a standard output protocol lets any tool inject a metric. GitHub Actions moved from standard output commands to `$GITHUB_OUTPUT` for the same reason. Decided 2026-11-22, replacing the earlier standard output proposal.
5. **One line per measurement.** The format is `metric [item] value`. An item with a space is quoted.
6. **Lighthouse scoring against one target.** A value at `target` scores 90, a value twice the target scores 50, and a value four times the target scores 10. When higher is better, half the target scores 50. The target is a requirement: meeting it starts the good band, and no second anchor makes missing it acceptable. Lighthouse's p10 control point becomes the target and its median becomes twice the target. Lists collapse to the p90. A category is the weighted mean of its scores. There is no overall score. Decided 2026-11-22, replacing the `target` and `limit` pair.
7. **Targets can gate CI.** `retros --require-targets` exits with status 1 when a metric's collapsed value is worse than its `target`, or when a metric with a target has no value because its script failed. The rule compares values, not rounded scores, so a value just past the target never passes as 90.
8. **Pure engine, thin command line.** `crates/engine` is deterministic and has no I/O. `crates/cli` runs scripts, reads and writes files, and drives Git.
9. **Built like Ondine and zyzz.** AGENTS.md, lints, toolchain pin, and testing rules follow those repositories. Work lands directly on `main`.

## The Contract

A project adds `retros.toml`. Each table is a category, `run` is the script that measures it, and every other key is a metric the script writes. Targets are optional. A metric with a target gets a score, and a metric without one shows its value and change only.

```toml
# Two categories, each measured by its own script.
[footprint]
run = "bun metrics/footprint.ts"
export-size = { target = "5kB" }
install-size = { target = "10MB" }

[speed]
run = "bun metrics/speed.ts"
import-time = { target = "20ms" }
```

The script appends lines to the file at `$RETROS_METRICS`:

```text
install-size 45.6MB
export-size viem:createClient 1.1kB
export-size viem/actions:readContract 31.7kB
bench-time "ABI Decode (500 results)" 1.82ms
```

- **Values and units:** `B`, `kB`, `MB`, and `GB` for sizes (decimal), `ns`, `µs`, `ms`, and `s` for time, no unit for counts, and `pass` or `fail` for checks. One metric uses one kind of unit.
- **Items:** a line with an item adds one entry to a list metric. Retros collects the entries, scores the collapsed value, and lists the worst entries.
- **Logs:** standard output and standard error are logs. Retros keeps the end of standard error for a failed run.
- **Failure:** a script that exits with a non-zero status gives its category no score.
- **Runs:** each script runs through `sh -c` from the directory that holds `retros.toml`, in its own process group. Retros kills the group when the script exits or times out, so background processes do not outlive the run.
- **Exit status:** 0 when every category passes, 1 when a category fails, and 2 when Retros cannot run, such as for an unknown argument or an invalid `retros.toml`.
- **Optional metric keys:** `weight` (default 1), `score` (how a list collapses: `p90` by default, or `median`, `max`, `min`, `sum`), and `better = "higher"` for metrics where more is better. A `better = "higher"` list collapses to its p10 by default, because its tail is at the low end.
- **Optional category keys:** `rounds` for timing metrics (default 5) and `timeout` (default `10m`).

## Scoring

1. **Repeat timings.** When a script's first run writes a time, Retros runs it `rounds` times and takes each timing item's median. Sizes, counts, and checks keep the first run's values.
2. **Collapse lists.** A list metric collapses to one value, the p90 by default. Percentiles take the nearest rank, so the p90 is a value the script wrote, with at most a tenth of the list above it. The p10 mirrors it.
3. **Score on the curve.** Lighthouse's `getLogNormalScore`, ported with its `erf` approximation. A value at `target` scores 90, a value at twice the target scores 50, smaller values approach 100, and larger values approach 0. Checks score the share of items that pass. A `better = "higher"` metric scores the reciprocal.
4. **Average categories.** Like Lighthouse, a metric's score is rounded to a whole number, and a category is the weighted mean of those scores, rounded. Metrics without a target are shown but not scored, and a metric with weight 0 is scored but left out of the mean. A failed metric drops out instead of counting as zero.

| Value against a 5 kB target | Score | Band |
| --- | --- | --- |
| 2.5 kB | 99 | good |
| 5 kB (the target) | 90 | good |
| 7.5 kB | 70 | needs work |
| 10 kB (twice the target) | 50 | needs work |
| 20 kB (four times the target) | 10 | poor |
| 27.5 kB (viem's p90 export) | 3 | poor |

A comparison reports change separately from score:

- **Exact units:** sizes, counts, and checks report any change.
- **Timings:** 8 interleaved base and head pairs. Retros reports a change when the medians differ by at least 5% and at least 7 of 8 pairs agree on its direction. Otherwise the change reads "within noise". The rule comes from an earlier zepto trial on one machine, so Phase 3 checks it on CI runners.
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
- [ ] `.agents/setup` that installs the pinned toolchain in Amp orbs.

**Gate:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass locally and in CI on `main`.

## Phase 1: Contract and Runner

Read the configuration, run each script once, and read what it wrote.

- [x] `engine::config`: typed `retros.toml`, unknown keys rejected, defaults applied, errors with line and column.
- [x] `engine::lines`: the metric line parser, with items, quoting, units, and checks.
- [x] `cli`: run each `run` command through `sh -c` in its own process group, from the directory that holds `retros.toml`, with a fresh `$RETROS_METRICS` file and a timeout that kills the group.
- [x] `retros check`: run each script once and confirm that every configured metric was written with one kind of unit. List metrics written but not configured.
- [x] Fixture projects under `crates/cli/tests/fixtures/`: passing, failing, malformed line, missing metric, and timeout.

**Gate:** integration tests cover each fixture. Every error message is snapshotted and reviewed, and names its file, line, and column.

## Phase 2: Scoring and Reports

Turn metric lines into scores and write the first reports.

- [x] Rounds and medians for timing metrics.
- [x] List roll-ups: `p90`, `median`, `max`, `min`, and `sum`.
- [x] The scoring curve, checks, `better = "higher"`, weights, and categories.
- [x] `retros` runs every category and `retros <category>` runs one.
- [x] Terminal summary, `.retros/report.json`, and `.retros/report.md`.

**Gate:** curve tests match values computed by Lighthouse's JavaScript and zepto's `bench/lighthouse.zig` for the same inputs. Report snapshots for a fixture project are reviewed.

## Phase 3: Comparison and Gates

Measure two revisions, report what changed, and fail CI on missed targets.

- [ ] `retros --compare <ref>` measures `<ref>` in a temporary Git worktree and the current checkout.
- [ ] Exact metrics run once per side. Timing metrics run 8 interleaved pairs, with the verdict rule above.
- [ ] Per-item changes, and a warning when `retros.toml` or a script it runs differs between the two revisions.
- [ ] The worktree is removed after success or failure.
- [ ] Changes appear in the terminal summary, `report.json`, and `report.md`.
- [ ] `--require-targets` exits with status 1 when a metric misses its target or has no value, and lists each miss. It works with and without `--compare`.

**Gate:** a fixture repository with a planted size regression reports that item. An unchanged revision reports no changes. A noisy timing fixture reads "within noise". A failing script still leaves no worktree behind. With `--require-targets`, a fixture that misses one target exits with status 1 and names that metric, and a fixture that meets every target exits with status 0.

## Phase 4: HTML Report

Write the report people open.

- [ ] One self-contained `.retros/report.html`: a gauge per category, score bands, metric rows, the worst items of each list metric, and changes when comparing.
- [ ] Light and dark schemes, phone layout, keyboard access, and reduced motion.

**Gate:** screenshots at desktop and phone widths, in light and dark schemes, are reviewed. The browser records no network requests.

## Phase 5: viem

Run Retros on a real TypeScript library.

- [ ] `metrics/footprint.ts`: export size for every export (esbuild without code splitting, gzip) and install size.
- [ ] `metrics/speed.ts`: import time for each of the 26 entrypoints.
- [ ] `metrics/types.ts`, if viem's development install works in an orb. Its postinstall builds contracts with Foundry.
- [ ] `retros.toml` in a local viem checkout. Nothing is committed to viem without approval.

**Gate:** with viem 2.58.4 and esbuild 0.28.2, the export-size list has 1,912 items and a p90 of 27.5 kB. `retros --compare` catches a planted regression in one module and reports nothing else beyond noise. The HTML report is reviewed.

## Phase 6: zepto

Replace zepto's harness lighthouse with Retros.

- [ ] Subjects: one set of scores per subject, such as each harness, rendered as tabs with alternatives collapsed. The design is decided at the start of this phase.
- [ ] zepto's scenarios become scripts that write metric lines. zc-mock, the harness commands, and the crash and resume scenario stay in zepto.
- [ ] GitHub Pages history, as zepto publishes it today.

**Gate:** on one machine and run, Retros reports the same category scores as `bench/lighthouse.zig` within noise. zepto then removes the scoring and report code from `bench/lighthouse.zig` and `bench/lighthouse.html`.

## Phase 7: Distribution

Make Retros easy to install and run in CI.

- [ ] Release binaries for Linux (x64 and arm64, glibc and musl) and macOS (x64 and arm64) on GitHub Releases.
- [ ] An install script and an npm package with platform packages, so `npx retros` works.
- [ ] A GitHub Action that runs `retros --compare` on pull requests and keeps one updating comment.
- [ ] The documentation site, following the Writing Documentation conventions.

**Gate:** clean Linux and macOS machines install and run Retros. The Action posts and updates a comment on a test pull request.

## Later

These are not scheduled: built-in measures (bundle size, startup, install size), templates and `retros init` with an agent prompt, re-measuring only items whose inputs changed, peak memory measurement, and Windows.

## Risks

- **Scripts can measure wrong.** An esbuild code-splitting mistake inflated small viem exports by up to 20×. Earliest signal: a value far from what an existing tool reports for the same thing. Later fix: built-in measures and planted controls.
- **List metrics are slow.** Export size for 1,912 viem exports took 174 s in an orb, and a comparison runs it twice. Earliest signal: a viem run over 10 minutes. Later fix: re-measure only changed items.
- **Timings are noisy on shared runners.** Earliest signal: most timing comparisons read "within noise". Fix: more pairs, or a dedicated runner.
- **Targets are guesses.** Earliest signal: every project lands in the same band. Fix: each project sets its targets and records where they came from.
