# Implementation plan

Uphill is continuous metrics and hill climbing. A project's scripts measure it, and Uphill scores each measurement against the target that fits it, remembers the accepted values, names the next step, and gates changes that slide back. This plan tracks Uphill from foundations to release. Each phase ends with an acceptance gate, and a phase is complete only when its gate passes.

## The Loop

Hill climbing is a loop, and every part of Uphill serves one step of it. A person or an agent runs the loop on each change.

| Step | Question | How Uphill answers | Status |
| --- | --- | --- | --- |
| Measure | What is the project like now? | Scripts write metric lines. Uphill runs them and repeats timings. | Done |
| Score | How far is it from its targets? | Each item scores on Lighthouse's curve against its own target. Metrics roll up into categories. | Done |
| Compare | What did this change move? | The baseline file holds the accepted sizes, counts, and checks beside their targets. Timings compare in interleaved pairs on one machine. | Done, CI runner check in Phase 6 |
| Next step | What should change next? | Opportunities rank changes by the category points they add. Notes say why an item misses. | Done |
| Gate | Can this change land? | `--require-baseline` fails on any change since the baselines, and `--require-targets` fails on a missed target. | Done |
| Remember | Where has the project been, and what moved it? | Baselines in Git, a history of runs, and metric blame. | Baselines done, history in Phase 9 |

## How Uphill Maps to Lighthouse

Uphill keeps Lighthouse's scoring and depth, and replaces its fixed browser gatherers with the project's own scripts.

| Lighthouse | Uphill |
| --- | --- |
| Category, such as Performance or Accessibility | A top-level key in `uphill.yaml`, named by the project |
| Audit, such as Largest Contentful Paint or image alt text | A metric |
| Audit details, such as each image without alt text | The metric's items, worst first |
| The p10 and median control points of an audit | The target and `poor`, which defaults to twice the target |
| Not applicable, manual, and informative audits | Checks that `skip` |
| Opportunities with estimated savings | Opportunities with the category points they add |
| Audit descriptions and help text | Notes on metric lines |
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
| Honest comparisons | Exact changes always show. Timing changes show only when repeated runs agree. | Baseline tests (done) and the Phase 6 gate |
| Agents climb | An agent runs the loop unattended: it reads the next step, changes code, and confirms the climb. | Phase 8 gate |
| Replace zepto's lighthouse | zepto's report comes from Uphill with the same scores within noise. | Phase 10 gate |
| One binary | A single static binary with no runtime to install. | Release assets in Phase 11 |

## Decisions

1. **Name.** Uphill. Repository `wevm/uphill`, binary `uphill`, configuration `uphill.yaml`, accepted values in each metric's `baseline`, and reports under `.uphill/`. The project was named Retros until 2026-11-23, when npm refused the unscoped name `retros` as too similar to `retry` and `metro`. npm accepts names at least 3 edits from popular packages, and `uphill` was free.
2. **Scripts measure with existing tools, Uphill scores.** Scripts run the tools a project already uses, such as Lighthouse, test runners, type checkers, linters, formatters, and benchmark harnesses, and turn their output into metric lines. A script measures a value itself only when no tool reports it. Uphill has no built-in measures until the same script recurs across projects, and scripts remain the escape hatch after that. Decided 2026-11-22, with the preference for existing tools added the same day.
3. **YAML or TOML configuration.** A project writes `uphill.yaml` or `uphill.toml`, and both parse into one tree of nodes with byte offsets, so every rule and error is shared and errors keep their line and column. YAML is the format most CI and agent configuration uses, and TOML is what Cargo users know. yaml-rust2's event parser adds four crates. Uphill writes baselines into either format in place. Decided 2026-11-23.
4. **Metrics go to a file.** Scripts append lines to the file at `$UPHILL_METRICS`, not to standard output. Tools that scripts call print to standard output, and a standard output protocol lets any tool inject a metric. GitHub Actions moved from standard output commands to `$GITHUB_OUTPUT` for the same reason. Decided 2026-11-22.
5. **One line per measurement.** The format is `metric [item] value [target <value>] [note <text>]`. An item or note with a space is quoted.
6. **One target, scored like Lighthouse.** A target is a requirement the project must meet. A value at its target scores 90, twice the target scores 50, and four times the target scores 10. When higher is better, half the target scores 50. Lighthouse's p10 control point becomes the target and its median becomes twice the target. Decided 2026-11-22, replacing the `target` and `limit` pair. It had no overall score until decision 49.
7. **Targets are contextual.** A target in `uphill.yaml` is the metric's default. A script that knows an item needs a different requirement writes the target on the line: a root entry point that bundles the whole library gets a larger target than a subpath, and a health endpoint gets a smaller one than a search endpoint. Each item is scored against its own target, and a list ranks its items by how far past their own targets they are, so a large item that meets a large target never hides a small item that misses a small one. When the project disagrees with a script, a pattern sets targets by item, such as `"viem/chains/*" = "1kB"`, and has the final say. A list where some items have a target and others do not, with no default, is an error. Decided 2026-11-22, with patterns added the same day. Since 2026-11-25 the targets live in the baseline file, and an item's own target there has the final say over patterns and lines (decision 36).
8. **Exact metrics and line targets have baselines, timings are compared.** Each metric's `baseline` records sizes, counts, and checks, which are the same on every machine, and the targets scripts set on their lines. Baselines are committed and reviewed like code, so accepting a regression or loosening a script's target is a visible change in review. Timings differ between machines, so their values are never recorded and compare only within one run on one machine. Decided 2026-11-22, with baselines moved into the configuration on 2026-11-23 (decision 21). Since 2026-11-25 the baseline file holds each item's own target, so a script that changes its line target no longer moves it (decision 36).
9. **Opportunities name the next step.** Uphill lists the changes that would raise each category's score most, in category points: one item meeting its target, one metric meeting its target, or a whole list when no single item moves the score. Gains under half a point are left out. Decided 2026-11-22.
10. **Gates compare values, and improvements lock in.** `--require-baseline` exits with status 1 when anything differs from the baselines, better or worse, or when no metric has one, like a snapshot test. The baselines on `main` then always match the code, every improvement becomes the new floor, and `--compare` can trust a revision's baselines. `--require-targets` exits with status 1 when a metric misses its target or has no value because its script failed. Both compare values, not rounded scores, so a value just past its target never passes as 90.
11. **Uphill owns repetition.** When a script's first run writes a time, Uphill runs it 5 times for a score, and Phase 6 runs 8 interleaved pairs for a comparison. No project tunes noise by hand. Decided 2026-11-22, removing the `rounds` key.
12. **TypeScript on Bun, no Python.** Scripts in this repository, including the dogfood scripts and the examples, are shell or TypeScript run by Bun. Projects that use Uphill pick any language. Decided 2026-11-22.
13. **Pure engine, thin command line.** `crates/engine` is deterministic and has no I/O. `crates/cli` runs scripts, reads and writes files, and drives Git.
14. **Built like Ondine and zyzz.** AGENTS.md, lints, toolchain pin, and testing rules follow those repositories. Work lands directly on `main`.
15. **Subjects.** A line may start with `subject:`, such as `zc: startup 0.64ms`, for projects that measure several harnesses, packages, or builds in one script. Each subject is scored on its own, reports compare subjects side by side, and a baseline records each subject under its name, such as `zc:`. A category may not mix lines with and without a subject, and a subject that does not write a metric has no score for it. Decided 2026-11-23 for zepto.
16. **Shared runs.** A top-level `run` and `timeout` apply to every category that sets none, and categories with the same `run` share one execution. Each line goes to the category that lists its metric, so one metric in two categories that share a run is an error. Decided 2026-11-23, so zepto's one runner feeds five categories.
17. **`varies`.** A size or count marked `varies = true`, such as peak memory, takes its median across rounds like a time, and its baseline records only its line target. Decided 2026-11-23.
18. **Descriptions.** Categories and metrics take a `description`, and every report shows it, so a reader knows what a number measures without opening `uphill.yaml`. Decided 2026-11-23.
19. **`.agents/`.** Uphill reads `uphill.yaml` or `uphill.toml`, at the project root or in `.agents/`, from the current directory or the nearest parent, and it is an error for one directory to have more than one. Scripts run from the project root, the directory that holds the configuration or `.agents`, so a configuration kept with a project's agent files still measures the whole project. Reports go to `.uphill/` in the project root. Decided 2026-11-23.
20. **Titles.** Categories and metrics take a `title`, such as `Time to first request`, which the HTML and Markdown reports show. Without one, the name shows in words: `request-out` reads `Request out`. The terminal, baselines, and metric lines keep names. Decided 2026-11-23.
21. **Baselines live in the configuration.** Each metric's `baseline` key holds what `uphill --update` accepted, next to its `target`, so one file holds a metric's requirement and its last accepted value. Uphill edits the file in place and keeps its comments and formatting. A baseline with items or subjects takes one line per entry, so a change shows as one line in a diff. When an edit would change anything else, Uphill writes nothing. This replaces `uphill.lock`. Decided 2026-11-23, and replaced by decision 36.
22. **`--compare` adds to a run.** `uphill --compare <ref>` measures `<ref>` in a temporary Git worktree with this checkout's categories and commands. Timings run 8 interleaved pairs, alternating which side runs first. Exact metrics compare with `<ref>`'s baselines, or with its run when it has none. The gates keep their meaning. Decided 2026-11-23.
23. **`setup`.** A top-level `setup` command, such as `bun install --frozen-lockfile`, runs once in each checkout Uphill creates, with the top-level `timeout`. The current checkout is assumed ready. A failed setup stops the run with status 2, and `setup` is no longer a category name. Decided 2026-11-23.
24. **Notes.** A line may end with `note <text>`, after any target, such as `note "hero.svg has no alt text"`. Scripts take the note from the tool that found the miss. Notes come from the first round, stay out of baselines, and show under their item in every report. Decided 2026-11-23.
25. **`--compact` for agents.** `uphill --compact` prints the scores on one line, each missed target with its note, the opportunities, and the changes, instead of each category's metrics. An agent reads it every pass, so it stays a few lines. `report.md` keeps the full detail for people. Decided 2026-11-23.
26. **One hand-written skill, for creating metrics.** `uphill init` sets a project up, `--compact` names the next step, and `uphill --compare HEAD` judges each change, so an agent climbs with the commands alone. `uphill skills add` installs skills generated from the commands and their help, one per command (decision 35). The commands cannot say which metrics are worth climbing, so `SKILL.md`, the self-named `uphill` skill, does: deep metrics for each surface, language and domain patterns such as TigerStyle and Lighthouse, and targets about 10 times better than the project is now. `uphill init` embeds it at the end of its brief, so the binary and the repository share one copy. Decided 2026-11-23, removing the `climbing-with-uphill` skill, amended 2026-11-24, and amended 2026-11-25 with jxom to add the skill, named `uphill` on 2026-11-25.
27. **Log and blame.** `uphill log` lists the commits that changed a metric's baseline. `uphill blame` names the last, or bisects commits measured in temporary worktrees when no baseline records the metric. A time is unchanged within noise of `HEAD` in interleaved pairs. Each commit runs its own configuration's `run`. Decided 2026-11-23.
28. **Runs live in Git notes.** `uphill --note` keeps a run's scores and values as a Git note on `HEAD` under `refs/notes/uphill`. Notes need no service, travel with the repository, and never expire like CI artifacts. CI on `main` writes and pushes them, with write access and a full clone. Decided 2026-11-23.
29. **Releases through Changesets.** A change that users see adds a changeset. The Main workflow keeps a version pull request open, and merging it releases: `binaries.yml` builds the archives, the Main workflow creates the `v<version>` GitHub Release, and Changesets publishes `uphill` to npm with the same binaries. `npm/version.ts` keeps the Cargo version equal to the npm version. One package with four binaries needs no npm scope, and an install downloads about 2.5 MB. Decided 2026-11-23, replacing tag-triggered releases and per-platform packages.
30. **`poor`.** A metric may set `poor`, the value that scores 50, Lighthouse's median control point. It defaults to twice the target, or half when higher is better, and an item with its own target scales it by that target over the default. Lighthouse's metrics use control points that are not a factor of 2 apart, such as 1.8 s and 3 s for mobile First Contentful Paint, so one target could not reproduce them. Decided 2026-11-23, extending decision 6.
31. **`skip`.** A check may be `skip` when it does not apply, such as an audit of video captions on a page without video. It counts toward neither side of a list, and a check whose every line skips has no score and stays out of its category, as Lighthouse gives a not applicable audit no weight. The terminal collapses skipped checks into a count. Decided 2026-11-23. A metric of any unit may write only `skip` lines when it does not apply, such as bundle sizes in a project without entry points, so a pack can give it a default target. Its targets go unchecked, and it has no score. Decided 2026-11-25.
32. **Lighthouse's arithmetic.** A curve score follows `Util.computeLogNormalScore`: scores above 0.9 gain up to half a point, and the score rounds down to a whole number. A category adds its metric scores as fractions of 1, in declared order, as `ReportScoring.arithmeticMean` does, so a mean of exactly half a point rounds the same way. Scores that rounded before can read one point lower. Decided 2026-11-23, so `examples/lighthouse` matches Lighthouse's reports exactly.
33. **Report notes.** `uphill --note` also keeps the run's `report.json`, byte for byte, as a note on `HEAD` under `refs/notes/uphill-report`. A dashboard across repositories then reads each noted commit's full report, with scores, targets, and titles, through the Git hosting API, without a clone, a token for public repositories, or rescoring. CI artifacts expire, need a token even on public repositories, and exist only on one CI, so they cannot serve that reader. A second ref keeps the compact notes small, since trends and `uphill log` read up to 1,000 of them. CI fetches and pushes both refs with the pattern `refs/notes/uphill*`. Decided 2026-11-23, extending decision 28.
34. **`uphill dashboard`.** `uphill dashboard <repository>...` reads the latest noted runs of several repositories and writes `.uphill/dashboard.html` in the current directory, with a summary in the terminal. A repository is a Git URL, a local path, or `owner/name` on GitHub. Uphill fetches the default branch as commits only, and both notes refs, into a temporary bare repository with Git's own credentials, then removes it. Each subject of each repository gets a row: the category scores of its newest noted run, each category's trend across the last 99 noted runs, and the metrics, targets, and opportunities of its report note, or the values of its compact note without one. The dashboard needs no host, makes no network requests, and reads private repositories without a token of its own. A hosted page and a page that fetches notes in the browser lost, since one needs a host and the other needs a token in the browser for private repositories. Decided 2026-11-23, building on decision 33.
35. **incur parses the command line.** `crates/cli` declares its commands with [incur](https://github.com/wevm/incur): `uphill [category]` as the root command and the others as a `Parser` enum. incur prints help, the version, the `--llms` manifest, and shell completions, so an agent reads every command and flag from the binary. Commands still write their own output and exit with 0, 1, or 2, and an argument that incur rejects goes to standard error with status 2. `uphill skills add` installs skills that incur generates from the commands. MCP stays off, since commands write to standard output. Decided 2026-11-24.
36. **Targets and baselines live in a baseline file.** `uphill.baseline.yaml` or `uphill.baseline.toml` sits beside the configuration, in its format, and mirrors its categories and metrics. It holds each metric's targets and the accepted sizes, counts, and checks, side by side and labelled, one line per item or subject (a list of checks lists only the items that differ from its default, decision 45), such as `viem/chains: { baseline: 61kB, target: 80kB }`. A key with `*` and only a target is a pattern, and `*` alone is the default, so `"*": { target: 5kB }` replaces `target: 5kB` and `"viem/chains/*": { target: 1kB }` replaces `targets`. A metric without items takes `bundle: { baseline: 27.5kB, target: 30kB }`, and `bundle: { target: 30kB }` sets a target before the first update. The short form `61kB target 80kB` read as two numbers without labels, so Uphill only reads it from older files. An item's own target in the file has the final say, then the first matching pattern, then the target on its line, then the default. `uphill --update` writes the target each item scored against as its own, so a line target or a pattern seeds an item the file does not list yet, and an edited target scores at once. The configuration keeps how each metric is measured and scored: `run`, `poor`, `weight`, `score`, `better`, `varies`, `title`, and `description`. Targets took most of the room in the configuration, and viem's baselines run to thousands of lines. Uphill writes the whole file, so its comments do not survive an update, and the in-place editor only removes the `target`, `targets`, and `baseline` keys of older configurations, including TOML dotted keys and tables of their own. Uphill still reads those keys, so older revisions compare and keep their `uphill log` history, and `uphill --update` moves them into the file. A record with only a target is not a baseline, so `--require-baseline` still asks for a first update. This replaces decision 21. Decided 2026-11-24 with jxom, and targets moved into the file on 2026-11-25 with jxom.
37. **Packs are copied, not extended.** `uphill add <pack> [category...] [--as <name>]` copies a pack's categories into the project's `uphill.yaml` and the scripts their `run` commands name beside it, and the project then owns them. A pack is any directory with an `uphill.yaml` or `uphill.toml`, so every project and example is one: a local directory, `owner/name/path` on GitHub, or a Git URL followed by `//path`, each with an optional `@ref`. Each category keeps its text and comments, takes the pack's top-level `run` and `timeout` when it sets none, and gets a comment that records the command that adds it again and the first 12 characters of the pack's commit. `uphill update` (first `uphill add --update`, a separate command since 2026-11-25 so that `add` only adds, as in pnpm) fetches the pack at that commit, from the history of the source's ref, and now, and merges three ways: a category or script that only one side changed takes that side, and one that both changed is a conflict that writes nothing unless `--force` takes the pack's version. A local directory is recorded from the project root and has no commit, so each difference is a conflict. Each example has a `README.md` that `add` prints, saying what to fit. A live `extends` key lost: it fetches at run time, so offline runs, `--compare`, and `uphill blame` at older revisions would depend on the network, and its merge rules would grow the contract. A pack's sizes, counts, and checks stay behind, and the default targets and patterns of its baseline file go into the project's baseline file once, for each metric that has none there. `uphill update` leaves the project's targets alone (decision 36). A YAML pack in a YAML project keeps its text. Any other pair of formats writes each category from its parsed keys, with the comments above the category and each key, so a YAML example goes into `uphill.toml` as tables and the reverse. Decided 2026-11-24 with jxom.

38. **Packs are pinned and vendored.** Proposed 2026-11-25, waiting on jxom. Replaces decision 37. A top-level `packs` key lists the packs a project runs. A remote entry is `owner/repo/path@<sha>` on GitHub or an `https` Git URL followed by `//path@<sha>`, and a remote entry without a full 40-character commit is an error. A trailing comment, such as `# main`, names the ref that `uphill update` follows. A local entry starts with `./` or `../`, and the repository's commit pins it. `{ pack, categories, install }` limits an entry to some categories or approves its install. Uphill fetches packs itself, with no package manager: `uphill add` and `uphill update` write each remote pack's files to `.agents/packs/<name>/`, which the project commits, so runs, `--compare`, and `uphill blame` read committed files and need no network. The fetch reads the commit from a temporary bare repository with `git ls-tree` and `git cat-file`, so Git writes no working tree and runs no hooks, filters, or submodules. It accepts only regular files, rejects links, `..`, `.git`, absolute paths, and unsafe names, caps sizes, and writes nothing unless every check passes. The commit must belong to a branch or tag of the named repository, be at least a day old unless `--allow-fresh`, and on update descend from the pinned commit. Uphill runs no pack code during `add` or `update`, ignores a pack's `setup`, and with `install: true` runs the frozen install for the pack's lockfile with `--ignore-scripts`. The engine scans each pack's text for capabilities, such as dynamic code, remote code, secrets, binaries, network hosts, executables, and environment names, and `update` prints their diff and asks for confirmation, or `--yes`. There is no content hash: `uphill audit` refetches each pin and compares it with the vendored files. Composition is unchanged: pack categories come after the project's own, two packs that declare one category are an error, and a project category with a pack category's name overrides it key by key, where `metric: false` drops a metric. Targets and accepted values stay in the project's baseline file (decision 36), and a pack's baseline file sets defaults beneath it. A package manager lost because jxom wants no package manager in the loop. The design and its sources are in the thread's "Pack security design" document.

    Security is the package manager's. The lockfile pins each version with its integrity hash, the package manager decides which install scripts run, and features such as `minimumReleaseAge`, provenance, and `audit` apply to packs like any dependency. A pack is code that runs with the user's permissions, so Uphill runs only the packs that `packs` lists. It never runs a package that is merely installed.

    Uphill publishes its packs as `@uphill/pack-<name>` from `packs/<name>/`, versioned with Changesets beside `uphill`. `uphill add`, `uphill update`, and the copying of decision 37 go away. Categories that an older `uphill add` copied stay as the project's own, and their comments do nothing. The examples stay projects to read, not packs. Copying lost when packs grew to tens of metrics: each pack put about 100 lines of another project's YAML in the configuration, each local change made the next update a merge conflict, and a pack's tools did not install with it. Vendored copies with Uphill's own pinning, hashes, and review of commands lost because they rebuild a package manager. A live `extends` that fetches at run time stays rejected (decision 37), because the lockfile and `setup` keep runs reproducible. Projects in other languages add a `package.json` for their packs, and they already need Bun to run the pack scripts. This replaces decision 37.

39. **Categories with the same name merge.** Decided 2026-11-25 with jxom. Replaces the composition part of decision 38. Packs share Lighthouse-style categories: `correctness`, `style`, `architecture`, `documentation`, `testing`, `configuration`, `commits`, `performance`, `accessibility`, and `best-practices`. One script can write several categories, which share its one run. Categories with the same name, from packs or the project, merge into one category with one score, the weighted mean of every metric. Each declaration keeps its own script, so a category holds a list of scripts, and each script routes its lines to the metrics its declaration lists. A metric that two packs declare in one category merges: the first pack's keys apply, and its items come from both scripts. Two scripts that write the same item, or the same metric without items, fail the category. A project category with a pack category's name takes the project's place in the order, runs its new metrics with its own `run`, overrides a pack metric key by key, and drops one with `metric: false`. It can no longer replace a pack's `run`, so the web pack runs the project's `build` script itself. Uphill measures one unit per script, groups units by command, and merges a category's units before checks for duplicates and scoring. A script that runs once beside one that runs five rounds repeats its round, which holds the same exact values. One category per pack lost: seven packs made seven unrelated scores, and a Rust and TypeScript project had two formatting checks in two places.
40. **Packs run the project's own linter and formatter.** Decided 2026-11-25 with jxom. Extends decision 2. A pack metric that a project's own tool already checks, such as `lint` and `formatted` in the TypeScript pack, runs that tool with the project's configuration. The script finds the tool from the project, not from the pack: Bun's `lint` and `fmt` when bunfig.toml has their section or a package.json script runs them, and else the first linter or formatter that package.json lists and `node_modules` holds. The pack's own tools run only when the project has none. A tool in package.json but not installed, or one that fails, skips the metric with the reason. Bun's tools come first because they read ESLint, oxlint, Prettier, and oxfmt configurations. A Bun without the command skips it, since `bun lint` there runs the project's `lint` script, which can call `bun lint` again. Metrics that check the pack's own conventions, such as `explicit-any`, keep the pack's oxlint with only the pack's rules, so the project's linter does not change them. The TypeScript pack's `prettier` metric goes away: a pack does not judge which tools a project uses. Always running the pack's oxlint and oxfmt lost because an ESLint, Biome, or Prettier project then failed rules it never chose, and its own configuration did nothing.
41. **A vendored pack keeps a `local/` directory for the project.** Decided 2026-11-26 with jxom. Extends decision 38. A pack whose scripts read project inputs, such as the TypeScript pack's `local/config.ts`, reads them from `.agents/packs/<name>/local/`, so a project keeps everything a pack needs in one place. `uphill update` moves `local/` into the new vendored copy, `uphill audit` leaves it out of the comparison, and `uphill remove` keeps it. A pack that ships `local/` fails the fetch checks, so a pack cannot overwrite the project's files. Inputs beside the pack, such as `.agents/typescript.ts`, lost because a project then kept one pack's files in two places.
42. **Vendored packs live in `.agents/uphill/packs/`.** Decided 2026-11-26 with jxom. Amends decisions 38 and 41. `uphill add` and `uphill update` write each remote pack to `.agents/uphill/packs/<name>/`, and runs read it there, so Uphill keeps its files under one directory in `.agents/`. Uphill reads no other place. A project that vendored packs in `.agents/packs/` moves that directory to `.agents/uphill/packs/`, and a run before the move names the new place in its error.
43. **The action's comment shows score gauges and links the HTML report.** Decided 2026-11-26 with jxom. Extends the GitHub Action and decision 22. The comment is one table, with a row per category and a gauge for the base branch and for the pull request, then a link to `report.html`, which the action uploads as an unzipped artifact that opens in the browser. The job summary keeps the whole `report.md`. GitHub shows only images that it fetches by URL, and it removes inline SVG and `data:` URLs, so `assets/gauges/` holds one SVG per score from 0 to 100 and one for a failed category, which `assets/gauges.ts` writes, linked on `raw.githubusercontent.com` on wevm/uphill's `main`. The engine renders the table as `.uphill/comment.md`, beside the reports, so a workflow that cannot use the action, such as one in an organization that allows only its own actions, posts the same comment. `comment.md` is the only report that links files outside itself, and the reports stay self-contained. To score the base branch, `--compare` runs every script of the revision at least once, even when its baseline file holds the exact values, and `report.json` lists the revision's scores under `comparison.scores`. That costs one more run of each script that writes no timing. A hosted image service, such as Socket's, lost because it adds a server and receives each score. Scores derived from the revision's baselines lost because a baseline file can be stale and holds no timings. A GitHub App that posts the comment lost for now: an organization owner still approves installing it, it holds standing write access on a wevm server, and fork pull requests already work with a `workflow_run` comment job. The thread's "Uphill GitHub App" document covers it.
44. **Packs are fetched into a cache, not vendored.** Decided 2026-11-26 with jxom. Replaces the vendoring of decisions 38, 41, and 42. The pinned entry in the configuration is the only file a project commits for a remote pack. A run fetches each pinned commit that is missing into `$XDG_CACHE_HOME/uphill/packs/<commit>/<path>/`, or `~/.cache/uphill/packs/`, with the fetch checks of `uphill add`: a branch or tag must reach the commit, and the tree must pass validation. The age check and the review stay in `add` and `update`. A commit names its tree exactly, so the cache holds one copy per machine, shared by projects and checkouts, and the approved install writes `node_modules` there. Run commands name the cached files by absolute path, so `--compare` runs the current checkout's pins in both revisions, and `uphill blame` fetches each revision's own pins. `uphill audit` goes away, because nothing in the project can drift from its pin. The `local/` directory goes away: a pack reads project inputs from `.agents/uphill/<name>/`, such as the TypeScript pack's `config.ts`. Vendoring lost because each pack put up to 440 kB of scripts and lockfiles in every project, each update was a large diff, and edited copies needed `audit`. Offline runs, its main gain, rarely held, since a pack with `install: true` needs the registry on a fresh checkout. A run now needs the network once per commit, and a pin whose commit the upstream deletes no longer fetches. A `--vendor` option lost because jxom wants one way.
45. **A list of checks records a default and its exceptions.** Requested 2026-11-26 by jxom, who has yet to review the choice of default. Amends decisions 8, 17, and 36. A check metric's items in the baseline file start with a default record, such as `"*": { baseline: pass }`, which holds the check of every item without its own record, and `uphill --update` writes only the items whose check differs from it. The default is the metric's most common check, with a tie going to `pass`, then `skip`, then `fail`, so a list where most files skip, such as an architecture lint that cannot read its program, lists its passes instead. A new item that matches the default is not new, and an item that differs from it compares with it, so a new failing item is `worse`. An item that the default covers and that is gone leaves no record to compare with, which is the cost of not listing it. `--compare` without the revision's baselines still lists every item of the revision's run, so such an item reports as gone there. `uphill --update` also leaves out an item's own target when it equals the pattern or default it falls back to, and the line does not set a different one. A line target that differs from the default still records, so the file holds it for editing. A check marked `varies`, such as the git pack's checks of the last 50 commits, records nothing, since each new commit is a new item and `--require-baseline` could never pass. It runs once, because a check has no median. Uphill reads the older files with one record per item, and `uphill --update` rewrites them. Leaving skips out lost because an unlisted item that starts to skip, or stops, would then report nothing. Always `pass` as the default lost because skip-heavy metrics, such as zepto's 229 skipped `shallow-module` items, then list every skip. A separate key that keeps a metric out of the baseline file lost to `varies`, which already means "records only its targets".
46. **Each metric's `description` states its rule, and `uphill rules` prints the rules.** Decided 2026-11-26 with jxom. Packs and AGENTS.md held the same conventions twice, and Uphill only scored code after an agent wrote it. A metric's `description`, already part of the contract, now states its rule as an instruction, such as "Suppress a lint with `#[expect(lint, reason = "...")]`, never `#[allow]`." Comments keep how the script measures. `uphill rules [category]` prints the composed categories as Markdown: each metric's description, then its default target and item patterns, without dropped metrics, so a project override of a pack's rule shows. `engine::rules` renders it. A metric that two packs declare joins their distinct descriptions, which amends decision 39, since each pack's rule holds for the items its script writes. Parsing YAML comments lost because comments are not data, and TOML and YAML keep them differently. A separate `rule` key lost because it repeats `description`, which reports already show. A path argument that prints only the rules for one file lost, because nothing in the configuration maps metrics to files.
47. **AGENTS.md @-mentions a rules file that Uphill writes.** Decided 2026-11-26 with jxom. `uphill rules --write` writes the rules of every category to `.agents/uphill/rules.md`, and puts one line at the top of AGENTS.md, under its title, that @-mentions the file. Without AGENTS.md, the line goes in CLAUDE.md, which agents read in its place, or else in a new AGENTS.md. Amp and Claude Code load an @-mentioned file with the guidance that mentions it, so every rule is in context and no prose in AGENTS.md changes. `uphill add`, `update`, and `remove` rewrite the file after they change the configuration, and `uphill check` fails while the file differs from `uphill rules`, which catches an edit to a description, an override, or a target. A marked block of pointer text inside AGENTS.md lost because the agent had to run `uphill rules` to see any rule, and Uphill had to edit prose that people own. Copying every rule into AGENTS.md lost because it rewrites a file that people edit. A skill lost because `uphill skills add` installs skills globally and agents load them on demand, while the rules apply to every change.
48. **A configuration may live in `.agents/uphill/`.** Decided 2026-11-26 with jxom. Extends the places that decision 1 and decision 37 name. Uphill reads `uphill.yaml` or `uphill.toml` at the project root, in `.agents/`, or in `.agents/uphill/`, for projects and packs alike. A configuration in `.agents/uphill/` belongs to the directory above `.agents`, so scripts run from the project root, and the baseline file sits beside it. The directory then holds every file Uphill reads or writes for a project: the configuration, its baseline file, the scripts, the rules file, and pack inputs. It is the default: `uphill init`, the guides, the examples, and this repository keep their measurement there. Uphill still reads `.agents/uphill.yaml` and the project root, so existing projects need no move.
49. **An overall score.** Decided 2026-11-27 with jxom. Amends decision 6. `report.json` holds a top-level `score`, the project's overall score from 0 to 100: the mean of the category scores, each subject of a category counted once. A failed category, or one without a score, drops out, as a failed metric drops out of its category. The mean sums scores as fractions of 1 in report order and rounds, like a category's mean (decision 32). Categories carry equal weight, since a category weight would need a new key. The action's `publish` writes it as `badges/score.json` and draws `badges/score.svg`, so a README shows one badge instead of one per category. Lighthouse has no overall score, and one number can hide a poor category behind good ones, so the category gauges stay in every report and the comment. A badge with every category score lost because jxom wants one number.

## The Contract

A project adds `uphill.yaml` or `uphill.toml`, at its root or in `.agents/`. Each top-level key is a category, `run` is the script that measures it, and every other key is a metric the script writes. The baseline file beside it, `uphill.baseline.yaml` or `uphill.baseline.toml`, holds the targets. A metric with a target gets a score, and a metric without one shows its value and change only.

```yaml
# Two categories, each measured by its own script.
footprint:
  run: bun metrics/footprint.ts
  # Gzipped bytes of each export.
  export-size: {}
  install-size: {}

speed:
  run: bun metrics/speed.ts
  import-time: {}
```

```yaml
# uphill.baseline.yaml: each metric's targets, and after `uphill --update`, each accepted value
# beside the target it scored against.
footprint:
  export-size:
    "*": { target: 5kB }
    "viem/chains/*": { target: 1kB }
  install-size: { target: 10MB }

speed:
  import-time: { target: 20ms }
```

The script appends lines to the file at `$UPHILL_METRICS`, each `[subject:] metric [item] value [target <value>] [note <text>]`. The values below are viem's from the earlier applied metrics trial, and the line target is an example:

```text
install-size 45.6MB
export-size viem:createClient 1.1kB
export-size viem/actions:readContract 31.7kB target 40kB
bench-time "ABI Decode (500 results)" 1.82ms
```

- **Values and units:** `B`, `kB`, `MB`, and `GB` for sizes (decimal), `ns`, `µs`, `us`, `ms`, and `s` for time, no unit for counts, and `pass`, `fail`, or `skip` for checks. One metric uses one kind of unit. A check that skips does not apply and has no score.
- **Items:** a line with an item adds one entry to a list metric. Uphill scores each entry, collapses the list to one score, and lists the entries worst first.
- **Targets:** `target <value>` at the end of a line sets that item's target, in the metric's kind of unit. It overrides the default, the first matching pattern overrides it, and the item's own target in the baseline file overrides both. In a pattern, `*` matches any run of characters. A target of 0 is an error.
- **Logs:** standard output and standard error are logs. Uphill keeps the end of standard error for a failed run.
- **Failure:** a script that exits with a non-zero status gives its category no score.
- **Runs:** each script runs through `sh -c` from the project root, the directory that holds `uphill.yaml` or `.agents`, in its own process group. Uphill kills the group when the script exits or times out, so background processes do not outlive the run.
- **Subjects:** a line that starts with `subject:` belongs to that subject, and each subject is scored on its own.
- **Optional metric keys:** `poor` (the value that scores 50, twice the target by default), `weight` (default 1), `score` (how a list collapses: `p90` by default, or `p10`, `median`, `max`, `min`, `sum`), `better = "higher"` for metrics where more is better, `varies = true` for a size, count, or check that differs between runs, `title`, and `description`. Older configurations set `target`, `targets`, and `baseline` too, which `uphill --update` moves into the baseline file. A `better = "higher"` list collapses to its p10 by default.
- **Optional category keys:** `timeout` (default `10m`), `title`, and `description`. A top-level `run` and `timeout` apply to every category that sets none, and categories with the same `run` share one execution.
- **Setup:** a top-level `setup` command prepares each checkout that `--compare` creates, such as by installing packages.

The command line:

```text
uphill [category] [--compare <ref>] [--compact] [--update] [--require-baseline] [--require-targets]
uphill check
uphill init
```

- **Baselines:** `uphill --update` writes each metric's targets and baseline to `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration, and both files are committed. Each item's baseline and target sit side by side, as in `viem/chains: { baseline: 61kB, target: 80kB }`, and a list of checks starts with a default record, such as `"*": { baseline: pass }`.
- **Files:** `.uphill/report.html`, `report.json`, and `report.md` are written on every run and ignored by Git.
- **Exit status:** 0 when every category passes, 1 when a category fails or a gate fails, and 2 when Uphill cannot run, such as for an unknown argument or an invalid `uphill.yaml`.

## Scoring

1. **Repeat timings.** When a script's first run writes a time or a metric marked `varies`, Uphill runs it 5 times and takes the median of each timing item and each varying item. Other sizes, counts, and checks come from the first run.
2. **Score each measurement.** Lighthouse's `getLogNormalScore`, ported with its `erf` approximation, scores each value against its own target and the metric's `poor`. Smaller values approach 100 and larger values approach 0. A `better = "higher"` metric scores the reciprocal. Like `Util.computeLogNormalScore`, scores above 90 gain up to half a point and then round down.
3. **Collapse lists.** Items rank by how far past their own target they are: value divided by target, or target divided by value when higher is better. The roll-up takes the item at its nearest rank, so the p90 is the item with at most a tenth of the list further past its target, and the median is the upper middle. The report names the selected item, such as `p90 9 kB (viem/op-stack)`. A `sum` scores the total of the values against the total of the targets. A check scores the share of items that pass.
4. **Average categories.** Like Lighthouse, a metric's score is a whole number, and a category is the weighted mean of those scores as fractions of 1, in declared order, rounded. Metrics without a target and checks that skip are shown but not scored, a metric with weight 0 is scored but left out of the mean, and a failed metric drops out instead of counting as zero.

| Value against a 5 kB target, with the default `poor` | Score | Band |
| --- | --- | --- |
| 2.5 kB | 99 | good |
| 5 kB (the target) | 90 | good |
| 7.5 kB | 70 | needs work |
| 10 kB (twice the target) | 50 | needs work |
| 20 kB (four times the target) | 10 | poor |
| 27.5 kB (viem's p90 export) | 3 | poor |

Changes are reported separately from scores:

- **Exact units:** sizes, counts, and checks compare with their baselines on every run, item by item, labeled worse, better, new, or gone.
- **Timings:** `--compare <ref>` runs 8 interleaved base and head pairs, and the side that runs first alternates between pairs. Uphill reports a change when the medians differ by at least 5% and at least 7 of 8 pairs agree on its direction. Otherwise the change reads "within noise". The rule comes from an earlier zepto trial on one machine, so Phase 6 checks it on CI runners.
- **Items:** per-item changes always show, so one regressed export is never hidden by the collapsed value.

## Phase 0: Foundations

Set up the repository, conventions, and checks that every later phase builds on.

- [x] Cargo workspace with `crates/engine` (pure library) and `crates/cli` (the `uphill` binary).
- [x] Rust 1.97.0 pinned in `rust-toolchain.toml`, `rustfmt.toml`, and workspace lints from Ondine.
- [x] `clippy.toml` purity bans for the engine, confirmed by a probe: `std::fs` and `HashMap` fail Clippy in `crates/engine` and pass in `crates/cli`.
- [x] AGENTS.md adapted from Ondine and zyzz, including the writing, documentation, and prose conventions, with `CLAUDE.md` linked to it.
- [x] README, MIT LICENSE, `.gitignore`, and this plan.
- [x] Integration test harness: `insta` inline snapshots and binary tests for `--version`, `--help`, and argument errors.
- [x] Verify workflow (format, Clippy, tests) on pushes to `main` and on pull requests.
- [x] `wevm/uphill` on GitHub with the first commit on `main`.
- [ ] `.agents/setup` that installs the pinned toolchain and Bun in Amp orbs.

**Gate:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass locally and in CI on `main`.

## Phase 1: Contract and Runner

Read the configuration, run each script once, and read what it wrote.

- [x] `engine::config`: typed `uphill.yaml`, unknown keys rejected, defaults applied, errors with line and column.
- [x] `engine::lines`: the metric line parser, with items, quoting, units, and checks.
- [x] `cli`: run each `run` command through `sh -c` in its own process group, from the directory that holds `uphill.yaml`, with a fresh `$UPHILL_METRICS` file and a timeout that kills the group.
- [x] `uphill check`: run each script once and confirm that every configured metric was written with one kind of unit. List metrics written but not configured.
- [x] Fixture projects under `crates/cli/tests/fixtures/`: passing, failing, malformed line, missing metric, and timeout.

**Gate:** integration tests cover each fixture. Every error message is snapshotted and reviewed, and names its file, line, and column.

## Phase 2: Scoring and Reports

Turn metric lines into scores and write the first reports.

- [x] Rounds and medians for timing metrics.
- [x] List roll-ups: `p90`, `p10`, `median`, `max`, `min`, and `sum`.
- [x] The scoring curve, checks, `better = "higher"`, weights, and categories.
- [x] `uphill` runs every category and `uphill <category>` runs one.
- [x] Terminal summary, `.uphill/report.json`, and `.uphill/report.md`.

**Gate:** curve tests match values computed by Lighthouse's `shared/statistics.js` at `896399bc` and zepto's `bench/metrics.zig` at `c8a97f7` for the same inputs. Report snapshots for a fixture project are reviewed.

## Phase 3: Baselines, Opportunities, and Contextual Targets

Remember accepted values, name the next step, and score each item against the target that fits it.

- [x] Per-line targets (`target <value>`), with errors for a zero target, a target in the wrong kind of unit, and a list that mixes lines with and without targets when the metric has no default.
- [x] Lists rank items by value against their own target, and reports name the selected item.
- [x] Baselines: sizes, counts, and checks sorted by name, written by `uphill --update`, with no timings, so the same code writes the same file on every machine.
- [x] Changes against the baselines, labeled worse, better, new, or gone, in the terminal summary, `report.json`, and `report.md`.
- [x] `--require-baseline` exits with status 1 when anything differs from the baselines, better or worse, or when no metric has one.
- [x] Baselines record the targets scripts set on their lines, including a timing's, and reports a changed one as looser or tighter.
- [x] `targets` in `uphill.yaml`: item patterns that have the final say over line targets, with errors for a target in the wrong kind of unit and warnings for a pattern that matches no item.
- [x] The `rounds` key is gone, and timings always run 5 times.
- [x] Opportunities: the top 5 in the terminal and the top 10 in `report.md`, each with the category points it adds.
- [x] Baselines move from `uphill.lock` into each metric's `baseline` key, which `uphill --update` writes into YAML or TOML in place (decision 21).

**Gate:** met. Engine tests cover line targets, patterns overriding them, ranking each item against its own target, mixed targets, and opportunity gains. CLI tests cover `--update` in YAML and TOML, baselines with line targets, an improvement alone failing `--require-baseline` until it is accepted, a planted regression and loosened target that rewrite only their own lines, a missing baseline, an invalid baseline, and a TOML form Uphill cannot edit.

## Phase 4: Examples and README

Show what a rich, Lighthouse-level setup looks like for common kinds of projects, and describe Uphill plainly.

- [x] Ban Python in AGENTS.md: scripts are shell or TypeScript on Bun. The dogfood `metrics/speed.py` is now `metrics/speed.ts`, and the verify workflow installs Bun 1.4.4.
- [x] Examples run real tools instead of hand-rolled measures: Lighthouse for the web app, TypeScript, oxlint, oxfmt, mitata, and Bun's test runner for the library, and clippy, rustfmt, and `cargo test` for the API.
- [x] `examples/typescript-library`: footprint (install size, dependencies, entry size, export size, bytes tree-shaking fails to drop, and an unscored export count its baseline tracks), speed (import time per entry point and mitata's median call time per function), and quality (tests, coverage per file, and type errors, lint, and formatting per file, and resolvable entry points). The root entry's target is the sum of its subpaths and 10% more, and formatters that call `Intl` get a larger call-time target by pattern.
- [x] `examples/web-app`: Lighthouse 13 in its default mobile mode on each page. Performance writes Lighthouse's five metrics and total byte weight per page. Accessibility, best practices, and SEO write each audit that counts toward Lighthouse's score as a check per page. The landing page gets larger paint and byte-weight targets by pattern.
- [x] `examples/rust-api`: footprint (binary size and dependencies), latency (median and 95th percentile per endpoint, and throughput), reliability (status codes, a malformed request, surviving a panic, and the error rate under load), and quality (clippy per file, an unscored count of panic sites from clippy's restriction lints, rustfmt per file, and each test). Each endpoint's target comes from the work it does, and its tail may take three times as long. Latency stays a fetch loop because autocannon under Bun reported inconsistent latencies and connection errors.
- [x] Each example misses a few targets on purpose, so its report shows misses, opportunities, and contextual targets.
- [x] README: the tagline "Continuous metrics and hill climbing", a Features section in the style of zyzz's README, "any project" under Philosophy, target-only scoring, targets, baselines, and usage.

**Gate:** CI runs `uphill check` in every example through the CLI tests. Every miss in each example's report names its item, its value, and its target. Waiting on review of the README and the example reports.

## Phase 5: viem

Run Uphill on a real TypeScript library, fanning out over every public export and function. jxom set this scope on 2026-11-23: rolldown, entry size for every export, no install size, CPU and memory, waterfalls, and types per export.

- [x] `footprint`: `entry-size` for each of the 1,916 runtime exports of the 26 entry points, and for each whole entry. rolldown bundles and minifies each import, and the size is gzipped. No export size and no install size. 1,942 items take about 2.5 minutes at concurrency 3.
- [ ] `speed`: `call-time` in CPU time and `call-memory` in heap bytes per call for every public function, replaying the arguments viem's tests call it with. `benched` fails for a function with no replayable call. `import-time` and `import-memory` for each entry point. On 2026-11-23, 297 of 1,331 functions were benched. Actions need anvil forks, the alto bundler, and the Tempo localnet for their tests to record calls.
- [x] `latency`: `waterfalls`, a check per async function that fails when it awaits independent work in sequence instead of in one `Promise.all`. Of 952 async functions, 7 fail, and a reading of each found all 7 real.
- [x] `types`: `instantiations`, `check-time`, and `check-memory` for every public export, values and types, each checked in its own program. viem's attest benches score against their pinned counts. 4,074 exports, with counts equal to `tsc --extendedDiagnostics`.
- [x] `uphill.yaml` with baselines in a local viem checkout. Nothing is committed to viem without approval. A second full run matched every baseline.

**Gate:** with viem 2.58.5, `entry-size` covers all 1,916 runtime exports. A planted regression in one module shows as worse in the items that bundle it, and nothing else changes. The top opportunities and waterfall findings name changes a viem maintainer agrees are worth making.

## Phase 6: Timing Comparisons and Target Gates

Compare timings honestly between two revisions, and fail CI on missed targets.

- [x] `uphill --compare <ref>` checks out `<ref>` in a temporary Git worktree. Timing metrics run 8 interleaved pairs with the verdict rule above. Exact metrics compare with `<ref>`'s baselines read from Git, and `<ref>` is measured only when it has none (decision 22). A top-level `setup` prepares the new checkout (decision 23).
- [x] A warning when `uphill.yaml`, apart from its baselines, or a file a `run` command names differs between the two revisions.
- [x] The worktree is removed after success or failure.
- [x] Timing changes appear in the terminal summary, `report.json`, `report.md`, and `report.html`.
- [x] `--require-targets` exits with status 1 when a metric misses its target or has no value, and lists each miss with its item. It works with and without `--compare`.
- [x] The verdict rule holds on a GitHub Actions runner: Uphill's CI compares each commit with itself and fails on any reported change. It read "No changes" on every push from 59aad1e to 2244a46.

**Gate:** an unchanged revision reports no changes. A planted slowdown is reported, and a noisy timing fixture reads "within noise", on both a local machine and a GitHub Actions runner. A failing script leaves no worktree behind. With `--require-targets`, a fixture that misses one target exits with status 1 and names that metric and item, and a fixture that meets every target exits with status 0.

## Phase 7: Details and HTML Report

Explain each miss, and write the report people open.

- [x] Notes: a line can say why an item misses, such as `alt-text /pricing fail note "hero.svg has no alt text"`. Notes appear under their items in every report (decision 24). The examples write the reasons their tools give: Lighthouse audit titles, oxlint and TypeScript messages, uncovered lines, and the status a request got.
- [x] Metric descriptions in reports, from the `description` key (decision 18).
- [x] One self-contained `.uphill/report.html`: a comparison table and one tab per subject, a gauge per category, score bands, opportunities, and changes. Each category shows its metrics as a grid, each value on a bar against its target, descriptions behind Expand view, and a collapsible row per list with its items worst first against their targets and their notes.
- [x] Titles for categories and metrics, from the `title` key (decision 20).
- [x] Light and dark schemes, phone layout, keyboard access to the tabs, and reduced motion.

**Gate:** the web app example's HTML report reads like a Lighthouse report: every miss names its item, its value, its target, and why. Screenshots at desktop and phone widths, in light and dark schemes, are reviewed. The browser records no network requests.

## Phase 8: Agents

Let the agent a person already works with set up Uphill and climb.

- [x] `uphill init` prints a brief for the user's agent: find the project's surfaces (entry points, pages, endpoints, or commands), write one script per category in the project's language or TypeScript on Bun that runs the tools the project already uses, set a target per item where context differs and record the reason beside it, and run `uphill check` until it passes. The brief points the agent at the closest example from Phase 4.
- [x] A compact report for agent loops: scores, misses, and the top opportunities in a few lines, from `--compact` (decision 25).
- [x] The loop needs no skill: `--compact` names the next step, and `uphill --compare HEAD` judges each change (decision 26).
- [x] The `uphill` skill guides the choice of metrics and ambitious targets, and `uphill init` ends its brief with it (decision 26).

**Gate:** an agent given only the brief sets up a project it has not seen until `uphill check` passes. Given an example's report, an agent raises a category score without making any metric worse.

On 2026-11-23, an agent given only the brief set up kleur until `uphill check` passed. Its scripts run kleur's own benchmark harness, uvu, c8, and npm. The run found gaps in the brief, which now says checks always score, targets are above zero, and timings run 5 times.

Another agent climbed the TypeScript library example from quality 95 to 99 in two commits, and `uphill --compare` against the start read nothing worse. It rejected a change that cut `call-time weekday` by 98%, because four sizes grew a few bytes within their targets.

## Phase 9: History and Blame

Remember where the project has been and what moved it.

- [x] Each run on `main` keeps its scores and values in a Git note under `refs/notes/uphill` (decision 28). `uphill --note` writes it, and CI pushes it.
- [x] `uphill log <category> <metric> [item]` prints a metric's values across commits. Exact metrics come from the Git history of their baselines, so they need no stored runs (decision 27). Other metrics, such as timings, come from notes.
- [x] `uphill blame <category> <metric> [item]` names the commit that moved a metric: from baseline history for exact metrics, and by measuring and bisecting otherwise (decision 27). `--subject` picks a subject, and `--since <ref>` limits the commits.
- [x] Each noted run also keeps its `report.json` under `refs/notes/uphill-report`, for dashboards that read many repositories (decision 33).
- [x] `uphill dashboard <repository>...` writes `.uphill/dashboard.html` from the latest noted runs of several repositories (decision 34).
- [x] Trends in the HTML report: each category and metric charts its last 100 runs, with its target and its change since the first, and names each run's commit, date, and value on hover or with the arrow keys.

**Gate:** on Uphill's own history, `uphill blame footprint dependencies` names the commit that added a dependency, and `uphill log` shows the change.

On 2026-11-23, blame measured 7 of 45 commits and named 78a79ab, which moved dependencies from 17 to 21 by adding yaml-rust2. `uphill log` shows nothing there, because Uphill commits no baselines of its own.

## Phase 10: zepto

Replace zepto's harness metrics report with Uphill.

- [x] Subjects, shared runs, and `varies` (decisions 15 to 17). The HTML report shows one tab per subject.
- [x] `zig build metrics` writes metric lines for each harness. zc-mock, the harness commands, and the crash and resume scenario stay in zepto, and its `uphill.yaml` holds the targets.
- [x] zepto's CI runs `uphill` and publishes `report.html` to GitHub Pages per run.
- [x] Pages history through Phase 9. Each metrics run on main keeps a note, and the Pages report charts every category and metric across the noted runs. On 2026-11-23, zepto's report for `5f3ecea` drew 56 trends from `5ed3e96`'s note and its own run.

**Gate:** Uphill scores zc, the Zig SDK, and the TypeScript package from one run of `zig build metrics` per round, and zepto removes its scoring and report code. Scores move because each metric has one target instead of zepto's `good` and `median` pair, which jxom accepted on 2026-11-23. On one orb, `uphill` took 48 s and scored the TypeScript package's speed 85, from 91, since its first request misses a 20 ms target.

## Phase 11: Distribution

Make Uphill easy to install and run in CI.

- [x] Release binaries for Linux (x64 and arm64, glibc and musl) and macOS (x64 and arm64) on GitHub Releases. `.github/workflows/binaries.yml` builds each target on its own runner and checks the binary's version against `package.json`. The Main workflow attaches one `uphill-<target>.tar.gz` per target and `sha256sums.txt` to the `v<version>` GitHub Release (decision 29). glibc builds link in a manylinux_2_28 container, so they run on glibc 2.28 and later. musl builds are static. A manual run builds and checks every archive without releasing, and the first one passed on 2026-11-23.
- [x] An install script and an npm package, so `npx uphill` works. `install.sh` picks the archive for the machine, takes the musl build on musl and on glibc older than 2.28, and checks its digest. The `uphill` package is the `npm/` workspace package, built with zile and versioned with Changesets from the zile template. It holds the macOS builds and the static Linux musl builds, and `npm/src/bin.ts` runs the one for the machine. The Main workflow packs it with pnpm and installs it on glibc and musl before it publishes. Through a local registry, `changeset publish` published it, and `npx` and `bunx` ran it on glibc and `npx` on Alpine. After publishing, the workflow runs `install.sh` and `npx uphill` on clean macOS and Linux runners, Alpine, AlmaLinux 8, and CentOS 7. `uphill@0.0.1` went out on 2026-11-23 through npm trusted publishing with provenance.
- [x] A GitHub Action that runs `uphill --compare` on pull requests and keeps one updating comment. `action.yml` installs the release that matches the action's tag, fetches the base branch when the clone lacks it, and runs `uphill --compare origin/<base>` with the `args` input. It writes `report.md`, and the output of a failed run, to the job summary and to one comment that it finds again by a marker. A token that cannot comment, such as a fork's, gives a warning, and the job exits with the status of `uphill`. The Pull Request workflow runs it on this repository with the `uphill` each pull request builds. Under act, the action fetched the base branch into a shallow clone, compared with it, and set its outputs. Posting the comment waits for a test pull request.
- [x] The documentation, following the Writing Documentation conventions. `docs/` holds Getting Started and Installation, guides for scripts, targets, baselines, comparing revisions, and GitHub Actions, and reference pages for the configuration, metric lines, units, scoring, reports, the CLI, and the action, as Markdown that GitHub renders. Each output on a page comes from a run of the release binary on the page's example. The pages started as a TanStack Start site in `site/`, which d0d9be0 has. A hosted site waits for a domain.

**Gate:** clean Linux and macOS machines install and run Uphill. The Action posts and updates a comment on a test pull request. Clean machines passed on 2026-11-23. After the 0.0.1 release, `install.sh` and `npx uphill@0.0.1` ran on macOS and Linux runners for arm64 and x64. `install.sh` also ran on Alpine, AlmaLinux 8, and CentOS 7. The comment waits for a test pull request.

## Phase 12: Packs

Compose a project's metrics from packs pinned to commits, vendored into the project, and checked before they run (decision 38).

- [x] Composition: the `packs` key in YAML and TOML, categories after the project's own, key-by-key overrides, and `metric: false`.
- [x] Shared categories (decision 39): the packs split into shared categories, categories with the same name merge with one script each, and the examples and this repository take the packs.
- [x] Engine: entries with required pins and `install`, `allowed-packs`, tree validation, the capability scanner in `engine::scan`, the review with its diff, and in-place edits of the `packs` key in YAML and TOML.
- [x] CLI: the hardened Git fetch in `crates/cli/src/fetch.rs`, vendoring to `.agents/uphill/packs/`, `uphill add`, `uphill update`, `uphill remove`, `uphill audit`, confirmation, and the approved frozen install before scripts run, also in `--compare` and `blame` checkouts.
- [x] Integration tests against real local Git repositories, reached through `url.<base>.insteadOf` in a test `GIT_CONFIG_GLOBAL`: a tampered vendored file, a symbolic link, a `..` path, a force-push, an impostor commit, an ambiguous ref, a fresh commit, new capabilities, an unpinned entry, `allowed-packs`, and an install without a lockfile.
- [x] Packs: rust, typescript, web, docs, git, tigerstyle, and deslop under `packs/`, each with a `README.md`, and a `package.json` and lockfile when it has tools. Scripts find tools beside themselves first and skip `.agents/uphill/packs/`.
- [x] Update the packs guide, the CLI and configuration references, the action's warning on `pull_request_target`, the `uphill init` brief, and `SKILL.md`.

**Gate:** a Rust project and a TypeScript project each add two packs from this repository, override one target and one metric, and pass `uphill check` offline. `uphill update` shows a capability diff, `uphill audit` fails on an edited vendored file, and `uphill --compare` against an older commit runs that commit's vendored packs. On 2026-11-25, in an orb, a copy of the TypeScript library example added the typescript, docs, and git packs from a local mirror of `packs/`, and `uphill check` passed 6 categories after installing each pack from its lockfile. `uphill update` showed the new `npx` and `GITHUB_TOKEN` findings, `uphill audit` reported an edited file, and `uphill --compare HEAD docs` ran the vendored docs pack in the checkout. Publishing the packs from this repository waits for a push.

## Phase 13: Agent Rules

Agents follow a project's metrics while they write code, not only after Uphill scores it (decisions 46 and 47).

- [x] Every pack metric states its rule as an instruction in `description`.
- [x] `uphill rules [category]` prints the composed rules with targets, from `engine::rules`.
- [x] The CLI and configuration references, the packs guide, the `uphill init` brief, and `SKILL.md` ask for a rule in each `description`.
- [x] `uphill rules --write`, `add`, `update`, and `remove` write `.agents/uphill/rules.md` and @-mention it at the top of AGENTS.md, and `uphill check` fails when the file is stale (decision 47).

**Gate:** `uphill rules` on this repository prints a rule for every pack metric, and an agent in a project with the rules file follows a pack rule without running Uphill.

## Later

These are not scheduled: built-in measures, such as `uphill time` and `uphill size` helpers that write metric lines, once the same script recurs across projects; re-measuring only items whose inputs changed; peak memory; and Windows.

## Risks

- **Scripts can measure wrong.** An esbuild code-splitting mistake inflated small viem exports by up to 20×. Earliest signal: a value far from what an existing tool reports for the same thing. Fix: scripts run established tools instead of hand-rolled measures, examples plant controls, and built-in measures come later.
- **Contextual targets can be loosened to pass.** Earliest signal: a pull request that raises a target in a script. Fix: done. Baselines record line targets, so a loosened one is a reviewed change.
- **`--update` edits the configuration.** Earliest signal: an update that changes a line outside a `baseline` key. Fix: done. Uphill parses the file it would write and writes nothing unless only baselines changed.
- **List metrics are slow.** Export size for 1,912 viem exports took 174 s in an orb. Earliest signal: a viem run over 10 minutes. Fix: partly done. `--compare` reads exact metrics from `<ref>`'s baselines instead of measuring it. Re-measuring only changed items comes later.
- **A stopped run leaves its worktree.** A signal ends `--compare` or `blame` before it removes its checkout. Earliest signal: `git worktree list` shows checkouts in the temporary directory. Fix: remove them on SIGINT and SIGTERM.
- **Timings are noisy on shared runners.** Earliest signal: most timing comparisons read "within noise". Fix: more pairs, or a dedicated runner.
- **Targets are guesses.** Earliest signal: every item lands in the same band. Fix: the agent brief asks for a reason beside each target, and reviews challenge targets like code.
