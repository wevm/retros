<h1 align="center">Retros</h1>

<p align="center">
  Continuous metrics and hill climbing
</p>

<p align="center">
  <a href="#overview">Overview</a> · <a href="#philosophy">Philosophy</a> · <a href="#features">Features</a> · <a href="#scoring">Scoring</a> · <a href="#usage">Usage</a> · <a href="#status">Status</a>
</p>

## Overview

Set a target for each metric that matters, measure every change, and climb. A project's scripts measure it, and Retros scores each measurement against its target, records what the project has reached, and names the next step.

```toml
# retros.toml: each table is a category, and `run` is the script that measures it.
[speed]
run = "bun metrics/speed.ts"
# Time to import each entry point. The root loads every module, so it gets more room.
import-time = { target = "2ms", targets = { "." = "3ms" } }
# Time per call of each formatter. Formatters that call `Intl` get more room than arithmetic.
call-time = { target = "500ns", targets = { "grouped" = "2µs", "weekday" = "2µs" } }
```

```sh
# A script appends one line per measurement to the file at $RETROS_METRICS.
echo 'call-time weekday 48.9µs' >> "$RETROS_METRICS"
```

```text
▲  49  speed  median of 5 rounds
  ●  98  import-time  p90 2 ms (.)           target 3 ms
                      2 ms    .  target 3 ms
                      1.3 ms  ./number
                      1.2 ms  ./date
  ▲   0  call-time    p90 48.9 µs (weekday)  target 2 µs
                      48.9 µs   weekday  target 2 µs
                      429.9 ns  bytes
                      408.4 ns  isoDate

Opportunities
  +45  speed  call-time weekday  48.9 µs → 2 µs
```

## Philosophy

- **Targeted.** Each metric has one `target`, a requirement the project must meet. Every change shows whether the project climbed toward its targets or slid back.
- **Contextual.** A root entry point, a landing page, and a health check need different requirements than their neighbors. Each item is scored against the target that fits it.
- **Any project.** Projects measure themselves with scripts in any language. Retros runs them and reads one line per measurement, so a library, a website, and an API all take part the same way.
- **Ratcheted.** Every improvement is recorded and becomes the new floor. With `--require-baseline` in CI, nothing changes without a reviewed change to `retros.lock`.
- **Honest.** Sizes, counts, and checks report every change. Timings take the median of 5 runs and stay out of the lock, because they differ between machines.
- **Portable.** One static binary, with no runtime to install.

## Features

- [**Lighthouse Scoring**](#scoring): every metric scores from 0 to 100 against its target on Lighthouse's curve, and each category averages its metrics.
- [**Contextual Targets**](#targets): a default per metric, a target per line when the script knows the context, and item patterns in `retros.toml` with the final say.
- [**Item Lists**](examples/web-app): any number of items per metric, each ranked against its own target, with the worst named in every report.
- [**Opportunities**](examples/typescript-library): the changes that would raise each category most, in category points.
- [**Baselines**](#baselines): `retros.lock` records every size, count, and check, and the targets scripts set, so any change shows in review.
- [**Gates**](#baselines): `--require-baseline` fails CI on any change that is not accepted, better or worse.
- [**Repeated Timings**](#scoring): timing scripts run 5 times, and each timing item takes its median.
- [**Reports**](#usage): a terminal summary, `.retros/report.json`, and `.retros/report.md`.

## Scoring

A value at its target scores 90, the start of the good band. A value twice the target scores 50, and four times the target scores 10. When `better = "higher"`, half the target scores 50. Bands follow Lighthouse: 90 to 100 is good, 50 to 89 needs work, and 0 to 49 is poor.

A metric that covers a list of items, such as every export or every page, ranks its items by how far each is past its own target and scores the item at its p90, the item with at most a tenth of the list further past its target. A check scores the share of items that pass. A category is the weighted mean of its metric scores, and there is no overall score.

When a script's first run writes a time, Retros runs it 5 times and scores each timing item's median. Sizes, counts, and checks come from the first run.

### Targets

A target comes from one of three places, and the later ones override the earlier ones:

1. `target` in `retros.toml` is the default for every item of the metric.
2. A script writes its own target at the end of a line when it knows the context, such as `latency /health 0.29ms target 500µs`.
3. `targets` in `retros.toml` sets targets by item pattern, such as `targets = { "viem/chains/*" = "1kB" }`. `*` matches any run of characters.

### Baselines

`retros --update` writes `retros.lock`: the accepted value of every size, count, and check, and each target a script set on a line. Commit it. Every run then lists what got worse, what got better, what is new or gone, and which targets got looser or tighter. Timing values stay out of the lock because they differ between machines.

`retros --require-baseline` exits with status 1 when anything differs from `retros.lock`, like a snapshot test. Accepting a change, even an improvement, is a reviewed change to the lock, so the lock on `main` always matches the code.

## Usage

Run Retros from the project or any directory below it, and add `.retros/` to `.gitignore`.

```sh
# Score every category, list the opportunities, and compare with retros.lock.
retros

# Score one category.
retros speed

# Record this run's sizes, counts, checks, and line targets in retros.lock.
retros --update

# Exit with status 1 when anything differs from retros.lock.
retros --require-baseline

# Run each script once and check what it writes, without scoring.
retros check
```

A metric line is `metric [item] value [target <value>]`. Values are sizes (`B`, `kB`, `MB`, `GB`), times (`ns`, `µs`, `ms`, `s`), counts with no unit, or checks (`pass` or `fail`). An item with a space is quoted. Each metric in `retros.toml` takes optional `target`, `targets`, `weight`, `score` (`p90`, `p10`, `median`, `max`, `min`, or `sum`), and `better = "higher"`. Each category takes `run` and an optional `timeout`.

[`examples/`](examples) has a TypeScript library, a web app measured like Lighthouse, and a Rust API. Retros also measures itself with the root [`retros.toml`](retros.toml).

## Status

> [!NOTE]
> Retros is in development. The [implementation plan](.agents/plan.md) lists each phase and its acceptance gate.

## License

[MIT](LICENSE)
