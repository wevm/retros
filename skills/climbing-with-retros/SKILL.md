---
name: climbing-with-retros
description: Raises a project's Retros scores one verified change at a time. Use when asked to improve, climb, or hill-climb Retros scores, fix what `retros` reports, or work through its opportunities. Not for setting Retros up in a project, which `retros init` covers.
---

# Climbing with Retros

Retros scores a project against the targets in its `retros.yaml` or `.agents/retros.yaml`. Each pass of the climb makes one change to the project's code and keeps it only when Retros shows that it helped and made nothing worse.

## Rules

- Raise scores by changing the project's code. Leave the metric scripts, `retros.yaml`, targets, and the targets scripts write on their lines as they are. What is measured and how much is required are the user's decisions, so propose such a change in the report instead of making it.
- A timing that `--compare` lists as within noise did not move. Only a metric that reads `better` counts as progress.
- Commit each kept change on its own, so the next pass compares against it.

## The loop

1. Start from a clean working tree. Ask the user before committing or stashing work that is not yours.
2. Run `retros --compact` and note each category's score.
3. Take the top opportunity, or the one the user named. Read its note in the missed targets, and its item in `.retros/report.md`, to learn why it misses.
4. Change the code to meet that target, in a change small enough to judge in one pass.
5. Run `retros --compact --compare HEAD`, which measures the working tree against the last commit.
6. Keep the change when the metric it targets reads `better` under "Against HEAD" and nothing reads `worse`. Otherwise restore the files the change touched, then try another approach or the next opportunity.
7. For a kept change, run `retros --update` when the configuration has `baseline` keys. Commit the code and the updated baselines together, with the metric and its values in the message, such as "Cache the weekday formatter: call-time weekday 1.4µs → 120ns".
8. Repeat from step 2 until no opportunity is left, the user's goal is met, or three approaches to one opportunity have failed. Then report each kept change with its before and after values, and each opportunity left with the reason.

## Gotchas

- `--compare HEAD` checks out the last commit in a temporary worktree. When the scripts need installed packages there, the configuration needs a top-level `setup` command, such as `setup: bun install --frozen-lockfile`. Ask the user before adding it.
- A comparison runs timings in 8 interleaved pairs, so it takes several times longer than one run. Name the category the change touches, such as `retros speed --compact --compare HEAD`, to compare only that one.
- Scores are rounded, so a real improvement can leave a category's score unchanged. The comparison is what decides.
