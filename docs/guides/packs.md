# Packs

A pack is a directory with an `uphill.yaml` or `uphill.toml` and the scripts its `run` commands name. A project lists packs under `packs`, and Uphill runs their categories after the project's own. Uphill pins each remote pack to a commit and vendors it into the project.

## Overview

`uphill add` fetches a pack at one commit, prints its files and what its code can do, and writes nothing until confirmed. The files go to `.agents/packs/<name>/`, and the entry takes the full commit. Runs read only these committed files, so they need no network.

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

```yaml title="uphill.yaml"
# The project's own category stays as it was.
footprint:
  run: bun metrics/footprint.ts
  bundle-size: {}

packs:
  - { pack: wevm/uphill/packs/typescript@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce, install: true } # main
```

### Run It

The first run installs the pack's dependencies in `.agents/packs/typescript/` from its lockfile, with install scripts turned off. `.agents/packs/.gitignore` keeps each `node_modules` out of Git. Commit the configuration and `.agents/packs/` together, so a review reads the pack's code.

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

### Take Some Categories

Names after the pack limit the entry to those categories. A project category with a pack category's name overrides it key by key, and `metric: false` drops one metric.

```yaml title="uphill.yaml"
packs:
  # Run only the pack's `typescript` category.
  - { pack: wevm/uphill/packs/typescript@94bd79a3bb8a413ff7080c08b2acec8b7c39e8ce, categories: [typescript], install: true } # main

typescript:
  # Weigh the lint check more, and drop the Prettier check.
  lint: { weight: 3 }
  prettier: false
```

### Update a Pack

`uphill update` moves each remote pack, or the named ones, to the newest commit of the ref in its comment, or of the default branch. The review shows what changed: `+` for a new file or capability, `~` for a changed file, and `-` for a removed one.

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

### Check the Pins

`uphill audit` fetches each remote pack at its pinned commit and compares it with the vendored files. It exits with status 1 when a vendored file differs from the pinned commit, or when either side lacks a file. Run it in CI.

```text
$ uphill audit
differs  wevm/uphill/packs/typescript at 47fd804
  metrics/typescript.ts differs
ok       wevm/uphill/packs/writing-docs at 94bd79a
ok       wevm/uphill/packs/git at 94bd79a
```

### Allow Sources

The top-level `allowed-packs` key lists patterns that every remote pack must match. A `*` matches any characters. `uphill add`, `uphill update`, and every run reject a pack that no pattern matches.

```yaml title="uphill.yaml"
# Only packs from the wevm organization.
allowed-packs: [wevm/*]
```

### Remove a Pack

`uphill remove` takes the pack's directory name. It removes the entry, and the vendored files of a remote pack.

```sh
# Remove the entry and .agents/packs/typescript/.
uphill remove typescript
```

### Write a Pack

A pack needs only its configuration and scripts. Write paths in `run` from the pack's directory, and Uphill rewrites them to start at the project root. Put tools in a `package.json` with a lockfile, and resolve them from the script's own directory first.

```text
packs/lighthouse/
  uphill.yaml         # Categories and targets, with a comment on each metric.
  metrics/
    lighthouse.ts
  package.json        # The tools the scripts run.
  bun.lock
  README.md           # What the pack measures and what to fit.
```

### Read the Checks

Each check stops `uphill add` or `uphill update` before it writes anything.

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
