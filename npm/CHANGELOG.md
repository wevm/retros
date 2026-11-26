# uphill

## 0.0.10

### Patch Changes

- 28f2a0c: Wrote each list of checks in the baseline file as a default record with its most common check, such as `"*": { baseline: pass }`, followed by only the items that differ from it, and left out each item target that equals the pattern or default that applies to it.
- eaf2d01: Read runtime built-ins such as `node:fs`, and modules that only `declare module` types, as outside the project in the deslop pack, so one import of them no longer skips every TypeScript file.
- 28f2a0c: Recorded no baseline for a check that sets `varies`, and set it on the git pack's commit checks, so a new commit no longer changes the baseline file.

## 0.0.9

### Patch Changes

- 7a2317b: Wrote `.uphill/comment.md` beside the reports: a heading and one table row per category, with a score gauge for the `--compare` revision and for the run. The GitHub Action posted it, and a workflow that could not use the action posted the same comment.

## 0.0.8

### Patch Changes

- 7533b9a: Fetched remote packs into a cache outside the project, `$XDG_CACHE_HOME/uphill/packs/` or `~/.cache/uphill/packs/`, instead of vendoring them into `.agents/uphill/packs/`. The configuration's pin is the only file a project commits, and a run fetches each pinned commit that the cache lacks, with the checks of `uphill add`. `uphill audit` and the vendored pack's `local/` directory went away. The TypeScript pack reads its inputs from `.agents/uphill/typescript/config.ts`. A project with vendored packs deletes `.agents/uphill/packs/` and moves `.agents/uphill/packs/typescript/local/` to `.agents/uphill/typescript/`.

## 0.0.7

### Patch Changes

- 7b1efe1: Replaced the GitHub Action's pull request comment with a table of score gauges, one row per category with its score on the base branch and on the pull request, and a link to `report.html`, uploaded as an unzipped artifact. The job summary kept the whole `report.md`. `uphill --compare` ran every script of the revision at least once, even with a baseline file, and `report.json` listed the revision's scores under `comparison.scores`.
- caab031: Fixed `uphill --update` in a project that overrode a pack's category. It read the configuration without its packs, so `metric: false` failed with "is not a pack's category" while a run without `--update` passed.

## 0.0.6

### Patch Changes

- ed127fc: Moved vendored packs from `.agents/packs/` to `.agents/uphill/packs/`. `uphill add` and `uphill update` wrote packs there, runs read them there, and the convention packs skipped it. A project with packs in `.agents/packs/` moved that directory with `git mv .agents/packs .agents/uphill/packs`.
- 7a2f0f1: Kept a vendored pack's `local/` directory for the project's own files. `uphill update` kept it, `uphill audit` skipped it, `uphill remove` left it in place, and a pack that shipped `local/` failed the fetch checks. The `typescript` pack read its project inputs from `local/config.ts`.
- 9576917: Corrected measurements across all seven convention packs and added parser dependencies that require `install: true` on existing Rust, TigerStyle, and Deslop entries.
- e914840: Added library categories to the `typescript` pack: `footprint`, `speed`, `coverage`, `types`, `call-types`, `type-coverage`, and `fuzz`. They measured each public export's bundle size and tree-shaking, the call and import cost of replaying the project's vitest calls, type instantiations per export and per test call, and fuzzing of each public function, with `hot-*` copies for the items its `local/config.ts` listed. A metric that wrote only `skip` lines no longer had its target's unit checked, so the library metrics skipped in projects without entry points.

## 0.0.5

### Patch Changes

- 282b69d: Measured the `deslop` pack in Rust, Zig, Go, Swift, C, and TypeScript. deslop-please read a Cargo workspace when it was installed, and a scanner read module cycles, tangles, leaked fields, and shallow modules in the other languages.
- d969835: Replaced copied packs with pinned, vendored ones. A top-level `packs` key listed packs such as `wevm/uphill/packs/rust@<commit>`, and Uphill ran their categories after the project's own. `uphill add` pinned a pack to a full commit and vendored it into `.agents/packs/<name>/` after a review of its files and capabilities. `uphill update` moved packs to newer commits that descend from the pinned ones, `uphill remove` removed them, and `uphill audit` compared each vendored pack with its pinned commit. Each fetch refused commits less than a day old, commits that no branch or tag contained, symbolic links, and unsafe paths, and ran no pack code. `allowed-packs` limited the sources, and `install: true` installed a pack's dependencies from its lockfile without install scripts. The repository gained the `deslop`, `git`, `rust`, `tigerstyle`, `typescript`, `web`, and `writing-docs` packs.
- 049ea92: Ran the project's own linter and formatter in the `typescript` pack. `lint` ran `bun lint`, ESLint, Biome, oxlint, or Vite Plus, and `formatted` ran `bun fmt`, Prettier, Biome, dprint, oxfmt, or Vite Plus, whichever the project used, with the project's configuration. The pack's oxlint and oxfmt ran only for a project with no linter or formatter. The `prettier` metric was removed.
- 10e057f: Merged categories with the same name. The packs moved their metrics into shared categories, such as `correctness`, `style`, `documentation`, and `testing`, and each category scored once across the project and its packs. A metric that two packs declared took the items of both scripts, and a project category added its own metrics with its own script. The web pack ran the project's `build` script before it read `dist/`.
- 513036e: Added the `uphill` skill to the end of the `uphill init` brief. It asked for deep metrics on each surface, research into language and domain patterns such as TigerStyle and Lighthouse, and targets about 10 times better than the project is now.

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
