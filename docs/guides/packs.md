# Packs

A pack is a directory with an `uphill.yaml` or `uphill.toml` and the scripts its `run` commands name. `uphill add` copies a pack's categories into a project and its scripts beside the project's configuration. The project then owns both.

## Overview

Any project is a pack, so a category written once runs in every project that adds it. The copy keeps each comment, so each metric comes with a description of what it measures. A comment above each category records the command and commit, so `uphill add --update` merges later changes.

```sh
# Copy the Lighthouse categories of the web-app example into this project.
uphill add wevm/uphill/examples/web-app
```

## Walkthrough

This walkthrough adds the `performance` category of the web-app example to a site that already measures its bundle.

### Add the Category

`uphill add` takes the pack and, optionally, the categories to add. It prints the pack's `README.md`, which says what to fit. A project without a configuration gets `.agents/uphill.yaml`, or `.agents/uphill.toml` from a TOML pack.

```text
$ uphill add wevm/uphill/examples/web-app performance
Added performance to .agents/uphill.yaml
Added their targets to .agents/uphill.baseline.yaml
Added .agents/metrics/

The pack prepares each checkout with `setup: bun install --frozen-lockfile`. Add it to .agents/uphill.yaml if the scripts need it

# Web App
...

Fit the scripts and targets to this project, then run `uphill check`
```

### Read the Result

The category's `run` points at the copied scripts. A script in a directory, such as `metrics/lighthouse.ts`, brings the whole directory, so the files it imports come too.

```yaml title=".agents/uphill.yaml"
# The project's own category stays as it was.
footprint:
  run: sh .agents/metrics/footprint.sh
  bundle-size: {}

# Added by `uphill add wevm/uphill/examples/web-app performance` at 03fa96e3deae.
performance:
  run: bun .agents/metrics/lighthouse.ts performance
  # Lighthouse's five metrics on each page, with Lighthouse's weights.
  first-contentful-paint: { weight: 10 }
```

The default targets and patterns of the pack's baseline file go into the project's baseline file, for each metric that has none there. The pack's accepted values stay behind.

```yaml title=".agents/uphill.baseline.yaml"
# The project's own target, then the targets the pack sets.
footprint:
  bundle-size: { target: 5kB }

performance:
  first-contentful-paint: { target: 800ms }
```

### Fit It

The targets and scripts are the pack's, so fit them to the project, such as its pages, its server command, and its packages. Then `uphill check` runs each script once, and `uphill --update` records the first baselines.

```sh
# 1. Check that each script writes the metrics the configuration lists.
uphill check
# 2. Record the first baselines in .agents/uphill.baseline.yaml.
uphill --update
```

## Recipes

### Pick a Source

A pack is a local directory, `owner/name/path` on GitHub, or a Git URL or repository path followed by `//path`. Uphill fetches a remote pack with Git's own credentials, so private repositories work.

```sh
# A local directory.
uphill add ../shared/uphill/rust-quality
# A directory in a GitHub repository.
uphill add wevm/uphill/examples/rust-api
# A directory in any Git repository.
uphill add https://gitlab.com/acme/metrics.git//packs/api
```

### Pin a Revision

`@ref` fetches a branch, a tag, or a commit. Without it, Uphill fetches the default branch, and the comment above each category records the commit either way.

```sh
# Add the quality category as it was at the v0.0.3 tag.
uphill add wevm/uphill/examples/typescript-library@v0.0.3 quality
```

### Update a Pack

`uphill add --update` fetches each added category's pack and compares three versions of each category and script: the pack at the recorded commit, the project, and the pack now. A version that only one side changed wins. The project owns its targets after `uphill add`, so an update leaves the baseline file as it is.

```text
$ uphill add --update
updated   performance from 03fa96e to 1a2b3c4
kept      seo from 03fa96e to 1a2b3c4, with this project's changes
updated   .agents/metrics/lighthouse.ts
```

### Resolve a Conflict

When the project and the pack both changed a category or script, the update writes nothing and exits with status 1. Merge it by hand, or pass `--force` to take the pack's version. A local directory records no commit, so each difference from it is a conflict.

```text
$ uphill add --update
conflict  performance  changed in both this project and the pack

Nothing was written. Merge by hand, or run `uphill add --update --force` to take the pack's version
```

### Mix Formats

A pack and a project can use different formats. A YAML pack goes into `uphill.toml` as tables, and a TOML pack goes into `uphill.yaml`. The comments above each category and each metric come too, and a comment at the end of a line stays behind.

```toml title="uphill.toml"
# Added by `uphill add wevm/uphill/examples/web-app seo` at 03fa96e3deae.
[seo]
title = "SEO"
run = "bun metrics/lighthouse.ts seo"
audits = {}
```

### Rename a Category

`--as` adds one category under another name, such as when the project already declares one with the same name. A name that the project already declares stops the command, and nothing is written.

```sh
# The project has its own quality category.
uphill add wevm/uphill/examples/rust-api quality --as rust-quality
```

### Share a Pack

A pack needs nothing beyond a project's own files. Keep its scripts in a directory beside the configuration, and write paths in `run` from the project root. `uphill add` prints the `README.md` beside the configuration, so it can tell an agent what to fit.

```text
packs/lighthouse/
  uphill.yaml         # Categories and targets, with a comment on each metric.
  metrics/
    lighthouse.ts
  README.md           # What to fit: pages, server command, and targets.
```

## More

- [Writing Scripts](./scripts.md): Write metric lines for lists, subjects, timings, and checks.
- [Baselines](./baselines.md): Record accepted values after the first run.
- [CLI](../reference/cli.md#uphill-add): Every argument and flag of `uphill add`.
