# Comparing Revisions

`uphill --compare <ref>` measures another revision on the same machine and reports what changed, including timings, which baselines cannot hold.

## Overview

Timings differ between machines, so a comparison needs both revisions measured in one place. `--compare` checks out `<ref>` in a temporary Git worktree, runs both revisions in interleaved pairs, and reports a timing change only when the pairs agree on it.

Sizes, counts, and checks are exact, so they compare with the baselines that `<ref>` committed. When `<ref>` has no baselines, Uphill measures it once. The changes appear in an "Against" section of every report.

```sh
# Compare the current checkout with the main branch.
uphill --compare main
```

## Walkthrough

This walkthrough times a `slugify` function in a Git repository. A branch adds accent removal, and the comparison shows how much slower each call gets.

### Measure the Function

The script times 100,000 calls and writes the time per call. A second category writes the size of the source, and its baseline is committed on `main`.

```yaml title="uphill.yaml"
# One category writes a size with a baseline, and one writes a time.
footprint:
  run: sh metrics/footprint.sh
  source-size: { target: 1kB, baseline: 138B }

speed:
  run: bun metrics/speed.ts
  slugify-time: { target: 500ns }
```

```ts title="metrics/speed.ts"
// Time 100,000 calls to slugify, and write the time per call.
import * as fs from 'node:fs'
import { slugify } from '../src/slugify.ts'

const start = performance.now()
for (let i = 0; i < 100_000; i++) slugify(`Crème brûlée ${i}`)
const time = ((performance.now() - start) * 1e6) / 100_000

fs.appendFileSync(process.env.UPHILL_METRICS!, `slugify-time ${time.toFixed(0)}ns\n`)
```

### Change the Code

The branch removes accents before it replaces other characters, which adds two passes over each string.

```ts title="src/slugify.ts"
// Normalize accented letters to their base letters before slugifying.
export function slugify(text: string): string {
  return text
    .normalize('NFKD') 
    .replace(/[\u0300-\u036f]/g, '') 
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
}
```

### Compare with Main

On the branch, `uphill --compare main` runs the `speed` category in 8 pairs, and alternates which revision runs first. The source size compares with the baseline on `main`, so Uphill does not measure it there.

```text
$ uphill --compare main
Comparing with main (ac66366)

● 100  footprint
  ● 100  source-size  172 B  target 1 kB

■  55  speed  median of 8 rounds
  ■  55  slugify-time  938.5 ns  target 500 ns

Opportunities
  +35  speed  slugify-time  938.5 ns → 500 ns

Since the baseline
  worse  footprint  source-size  138 B → 172 B

Against main (ac66366)
  worse  footprint  source-size   138 B → 172 B
  worse  speed      slugify-time  440 ns → 938.5 ns (+113%)  8 of 8 pairs
```

### Read the Verdict

A timing changes only when the medians differ by at least 5% and at least 7 of 8 pairs agree on the direction. Otherwise it reads "within noise". A comparison never fails the run by itself, so a gate needs `--require-targets` or `--require-baseline`.

After the branch accepts its new size with `uphill --update`, a comparison of the branch with itself reports no change:

```text
$ uphill --compare normalize
...
No changes against normalize (fd268f9)
  1 timing within noise
```

## Recipes

### Prepare Checkouts

A new worktree has only the files Git tracks. A top-level `setup` command, such as a package install, runs once in each worktree that `--compare` or `uphill blame` creates, before any script. The current checkout is assumed ready. A failed setup stops the run with status 2.

```yaml title="uphill.yaml"
# Install packages in each worktree that Uphill creates.
setup: bun install --frozen-lockfile

speed:
  run: bun metrics/speed.ts
  slugify-time: { target: 500ns }
```

### Gate on Targets

`--require-targets` exits with status 1 when a metric misses its target or a script fails, and lists each miss. It works with and without `--compare`.

```text
$ uphill --compare main --require-targets
...
Missed targets
  speed  slugify-time  917 ns  target 500 ns

1 target missed
```

### Compare Any Revision

`<ref>` is any revision Git resolves, such as a branch, a tag, or a commit. Both revisions run the categories and commands of the current checkout.

When `uphill.yaml`, apart from its baselines, or a file that a `run` command names differs in `<ref>`, Uphill prints a warning. Here, the current checkout has a tighter target:

```text
$ uphill --compare v1.2.0
Comparing with v1.2.0 (ac66366)
warning: uphill.yaml differs in v1.2.0 (ac66366). Both revisions run the categories and commands of this checkout
```

### Find the Commit

`uphill blame` finds the commit that moved a timing. It measures commits in temporary worktrees and bisects for the first one within noise of `HEAD` in interleaved pairs. `--since <ref>` limits the search, such as to the commits since the last release.

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

### Compare in CI

The GitHub Action compares each pull request with its base branch and posts the report as a comment. A clone needs the base revision, and the action fetches it when the clone lacks it.

```yaml title=".github/workflows/uphill.yml"
# Compare with the base branch, which is the default on pull requests.
- uses: wevm/uphill@main
```

## More

- [GitHub Actions](./github-actions.md): Compare pull requests and keep one comment.
- [Baselines](./baselines.md): Record accepted sizes, counts, and checks.
- [Reports](../reference/reports.md): The comparison in report.json and report.md.
- [CLI](../reference/cli.md): Every flag and exit status.
