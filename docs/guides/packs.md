# Packs

A pack is a directory with an `uphill.yaml` and the scripts its `run` commands name. `uphill add` copies a pack's categories into a project and its scripts beside the project's configuration. The project then owns both.

## Overview

Any project is a pack, so a category written once runs in every project that adds it. The copy keeps each comment, so each metric arrives with the description of what it measures. A comment above each category names the pack and the commit it came from.

```sh
# Copy the Lighthouse categories of the web-app example into this project.
uphill add wevm/uphill/examples/web-app
```

## Walkthrough

This walkthrough adds the `performance` category of the web-app example to a site that already measures its bundle.

### Add the Category

`uphill add` takes the pack and, optionally, the categories to add. A project without a configuration gets `.agents/uphill.yaml`.

```text
$ uphill add wevm/uphill/examples/web-app performance
Added performance to .agents/uphill.yaml
Added .agents/metrics/

The pack prepares each checkout with `setup: bun install --frozen-lockfile`. Add it to .agents/uphill.yaml if the scripts need it

Fit the scripts and targets to this project, then run `uphill check`
```

### Read the Result

The category's `run` points at the copied scripts. A script in a directory, such as `metrics/lighthouse.ts`, brings the whole directory, so the files it imports come too.

```yaml title=".agents/uphill.yaml"
# The project's own category stays as it was.
footprint:
  run: sh .agents/metrics/footprint.sh
  bundle-size: { target: 5kB }

# Added by `uphill add wevm/uphill/examples/web-app@03fa96e`.
performance:
  run: bun .agents/metrics/lighthouse.ts performance
  # Lighthouse's five metrics on each page, with Lighthouse's weights.
  first-contentful-paint: { target: 800ms, weight: 10 }
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

### Rename a Category

`--as` adds one category under another name, such as when the project already declares one with the same name. A name that the project already declares stops the command, and nothing is written.

```sh
# The project has its own quality category.
uphill add wevm/uphill/examples/rust-api quality --as rust-quality
```

### Share a Pack

A pack needs nothing beyond a project's own files. Keep its scripts in a directory beside `uphill.yaml`, and write paths in `run` from the project root. A `README.md` beside the configuration is printed after the copy, so it can tell an agent what to fit.

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
- [CLI](../reference/cli.md#uphill-add): Every argument of `uphill add`.
