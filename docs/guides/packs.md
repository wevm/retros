# Packs

A pack is a directory with an `uphill.yaml` or `uphill.toml` and the scripts its `run` commands name. A project lists packs under `packs`, and Uphill runs their categories after the project's own, merged by name. Uphill pins each remote pack to a commit and fetches it into a cache outside the project.

## Overview

`uphill add` fetches a pack at one commit, prints its files and what its code can do, and writes nothing until confirmed. The entry takes the full commit, and the project commits only that entry. A run fetches each pinned commit that the cache lacks.

```sh
# Add the Rust conventions pack from the main branch of this repository.
uphill add wevm/uphill/packs/rust@main
```

## Walkthrough

This walkthrough adds the `typescript` pack to a library that already measures its bundle.

### Add the Pack

`uphill add` takes the pack and an optional `@ref`: a branch, a tag, or a full commit. It checks the commit, then prints a review. The review lists each file and each capability the scan finds, with the place it first appears.

```text
$ uphill add wevm/uphill/packs/typescript@main
typescript  wevm/uphill/packs/typescript@94bd79a (main, 6 days old)
files  5 (58 kB)
  README.md
  bun.lock
  metrics/typescript.ts
  package.json
  uphill.yaml
capabilities
  info  dependency    @types/bun@1.4.5    package.json
  info  dependency    oxc-parser@0.160.0  package.json
  info  dependency    oxlint@1.95.0       package.json
  info  environment   UPHILL_METRICS      metrics/typescript.ts:218
  info  process       Bun.spawn           metrics/typescript.ts:226
  info  process       git                 metrics/typescript.ts:1137
  info  process       bun                 uphill.yaml:11

Its dependencies install from its lockfile, without install scripts, before its categories run

Add this pack? [y/N]
```

### Read the Result

The entry pins the full commit, and the comment names the ref that `uphill update` follows. The pack has a `package.json`, so the entry approves its install with `install: true`.

```yaml title=".agents/uphill/uphill.yaml"
# The project's own category stays as it was.
footprint:
  run: bun .agents/uphill/metrics/footprint.ts
  bundle-size: {}

packs:
  - { pack: wevm/uphill/packs/typescript@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce, install: true } # main
```

### Run It

The first run installs the pack's dependencies from its lockfile in the cached pack, with install scripts turned off. Commit the configuration, and the [rules file](../reference/cli.md#the-rules-file) and AGENTS.md that `uphill add` wrote. A review of the commit reads the pin, and `uphill add` printed the pack's code for review.

```sh
# 1. Check that each script writes the metrics the configuration lists.
uphill check
# 2. Record the first baselines.
uphill --update
```

## Recipes

### Pick a Source

A remote pack is `owner/repo/path` on GitHub or an `https` URL followed by `//path`. Git fetches it with its own credentials, so private repositories work. A local pack starts with `./` or `../`, and the project's own commit pins it.

```sh
# A directory in a GitHub repository, at a tag.
uphill add wevm/uphill/packs/web@v0.1.0
# A directory in any Git repository over https.
uphill add https://gitlab.com/acme/metrics.git//packs/api@main
# A directory in this repository.
uphill add ./packs/api
```

### Merge Categories

Packs share categories in the manner of Lighthouse, such as `correctness`, `style`, `documentation`, and `testing`. Categories with the same name merge into one score, the weighted mean of every metric. Each pack's script runs once.

```yaml title=".agents/uphill/uphill.yaml"
# The rust and typescript packs both write `style`, so the report shows one Style score.
packs:
  - wevm/uphill/packs/rust@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce # main
  - { pack: wevm/uphill/packs/typescript@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce, install: true } # main
```

A metric that two packs declare in one category, such as `formatted`, is one metric with the items of both scripts. The first pack's keys apply, and the metric's description joins each pack's distinct rule. Two scripts that write the same item fail the category.

### Adjust Categories

A project category with a pack category's name merges with it, in the project's place. Its new metrics run with its own `run` command. A key on a pack metric overrides that key, and `metric: false` drops the metric.

```yaml title=".agents/uphill/uphill.yaml"
# The project's test script joins the packs' `testing` category.
testing:
  run: bun .agents/uphill/metrics/tests.ts
  passing: {}
  # Weigh the packs' inline snapshot check more, and drop their snapshot file check.
  inline-snapshots: { weight: 3 }
  snapshot-files: false
```

### Take Some Categories

Names after the pack limit the entry to those categories.

```yaml title=".agents/uphill/uphill.yaml"
packs:
  # Run only the pack's `style` and `documentation` categories.
  - { pack: wevm/uphill/packs/typescript@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce, categories: [style, documentation], install: true } # main
```

### Update a Pack

`uphill update` moves each remote pack, or the named ones, to the newest commit of the ref in its comment, or of the default branch. The review shows what changed: `+` for a new file or capability, `~` for a changed file, and `-` for a removed one.

If an update adds dependencies, review the lockfile and set `install: true` on the pack entry. The Rust, TigerStyle, and Deslop packs need this setting for their parsers. An update preserves the existing install permission.

```text
$ uphill update typescript
typescript  94bd79a → 47fd804 (main, 3 days old)
files  +1 ~1 -0 (58.1 kB)
  + metrics/upload.ts
  ~ uphill.yaml
capabilities
+ high  remote code   npx                 metrics/upload.ts:8
+ high  secret        GITHUB_TOKEN        metrics/upload.ts:7
  info  dependency    @types/bun@1.4.5    package.json
  info  dependency    oxc-parser@0.160.0  package.json
  info  dependency    oxlint@1.95.0       package.json
  info  environment   UPHILL_METRICS      metrics/typescript.ts:218
  info  process       Bun.spawn           metrics/typescript.ts:226
  info  process       git                 metrics/typescript.ts:1137
+ info  process       npx                 metrics/upload.ts:8
  info  process       bun                 uphill.yaml:11

Write this update? [y/N]
```

### Cache Packs

Each pinned commit lives in `$XDG_CACHE_HOME/uphill/packs/`, or `~/.cache/uphill/packs/` without `XDG_CACHE_HOME`. A commit names its files exactly, so every project and checkout on the machine shares one copy. A run fetches a missing commit with the checks of `uphill add`, apart from its age.

```text
~/.cache/uphill/packs/
  94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce/
    packs@typescript/  # wevm/uphill/packs/typescript at this commit.
      metrics/
      node_modules/    # The install that `install: true` approves.
      uphill.yaml
```

`uphill --compare` runs the commands of the current checkout in both revisions, so both run the packs that the current checkout pins. A change to a pin then does not move the comparison.

### Allow Sources

The top-level `allowed-packs` key lists patterns that every remote pack must match. A `*` matches any characters. `uphill add`, `uphill update`, and every run reject a pack that no pattern matches.

```yaml title=".agents/uphill/uphill.yaml"
# Only packs from the wevm organization.
allowed-packs: [wevm/*]
```

### Keep Project Files

A pack that reads project inputs reads them from `.agents/uphill/<name>/` in the project, which the project commits. The pack's own files stay in the cache.

```text
.agents/uphill/typescript/
  config.ts           # The project's inputs, which the typescript pack's scripts read.
```

### Remove a Pack

`uphill remove` takes the pack's directory name, and removes its entry.

```sh
# Remove the typescript entry from the configuration.
uphill remove typescript
```

### Write a Pack

A pack needs only its configuration and scripts. Give each metric a `description` that states its rule as an instruction, so [`uphill rules`](../reference/cli.md#uphill-rules) prints it for agents before they write code. Keep comments for how the script measures. Write paths in `run` from the pack's directory, and Uphill rewrites them to start at the project root. Use the shared category names. Put tools in a `package.json` with a lockfile, and resolve them from the script's own directory first.

```text
packs/lighthouse/
  uphill.yaml         # Categories and metrics, with a rule in each metric's description.
  metrics/
    lighthouse.ts
  package.json        # The tools the scripts run.
  bun.lock
  README.md           # What the pack measures and what to fit.
```

### Read the Checks

Each check stops `uphill add` or `uphill update` before it writes anything. A run that fetches a pin applies the checks of the fetch itself.

| Threat | Check |
| --- | --- |
| A moved branch or tag | Every remote entry holds a full commit. |
| A commit that only a fork has | A branch or tag of the named repository must contain the commit. |
| A ref that names a branch and a tag | `uphill add` rejects it. |
| A force-push | `uphill update` takes only a commit that descends from the pinned one. |
| A malicious commit not yet caught | A commit must be at least a day old, unless `--allow-fresh`. |
| Code from other hosts | Sources are GitHub paths or `https` URLs. A pack's own `packs` key is an error. |
| Commands run by Git | Git fetches into a bare repository, so no hooks, filters, LFS, or submodules run. |
| Links and paths out of the pack | Only regular files, with plain names, under 1 MB each and 10 MB in all. |
| Install scripts | Uphill runs no pack code during `add` or `update`, and installs with `--ignore-scripts`. |
| Hidden text | Uphill removes control, bidirectional, and zero-width characters from pack text it prints. |

## More

- [Writing Scripts](./scripts.md): Write metric lines for lists, subjects, timings, and checks.
- [Baselines](./baselines.md): Record accepted values after the first run.
- [CLI](../reference/cli.md#uphill-add): Every argument and flag of the pack commands.
