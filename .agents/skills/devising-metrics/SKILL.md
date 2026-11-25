---
name: devising-metrics
description: Devises Uphill metrics and targets for a project. It researches language patterns, such as TigerStyle, and domain patterns, such as Lighthouse. It measures every surface in depth, with targets about 10 times better than the project is now. Use to write or review `uphill.yaml`, `uphill.baseline.yaml`, or the scripts in `.agents/metrics/`.
---

# Devising Metrics

A good metric moves with work that users feel, and its value is exact or repeatable. Its target asks for a better project, not the current one. Uphill ranks opportunities by score, so the metrics and targets decide where an agent spends its effort.

Do not ask for small wins such as "optimize X". Ask for "make X 10 times faster", and design the metrics so that the climb is visible at each step.

## Process

1. **Map the surfaces:** List what users import, open, call, or run, such as entry points, exports, pages, endpoints, commands, and queries. Each surface becomes an item, not a part of a total.
2. **Research the patterns:** Read the project's AGENTS.md, then the published patterns of its language and domain (see [Patterns](#patterns)). Use the domain's names for metrics, so a reader recognizes them.
3. **Sketch the physics:** Estimate the floor for each surface from the four resources (network, storage, memory, compute) and their two properties (bandwidth, latency). For example, a 4 kB response on a local socket needs microseconds, not milliseconds. The gap from the floor to the current value is the room to climb.
4. **Choose metrics in depth:** Measure each surface on several axes (see [Depth and Detail](#depth-and-detail)).
5. **Set ambitious targets:** Derive each target from the floor, the best known implementation, or a domain threshold made stricter (see [Ambitious Targets](#ambitious-targets)).
6. **Write the reasons:** Give each metric a comment in `uphill.yaml` that says what it measures and where its target comes from. Cite the source, such as a Lighthouse control point or a TigerStyle rule.
7. **Prove each metric:** Run `uphill check`, then `uphill`. Break each new metric once, such as with an extra import or a sleep. Make sure that its score falls. A metric that cannot fail measures nothing.

## Ambitious Targets

A target is a requirement that the project must meet, not a record of where the project is. `uphill --update` records the current value as the baseline, so the target is free to describe a better project.

- **About 10 times better:** Start from a tenth of the current value. If the physical floor is nearer, start from the floor. "10 times" is a direction, not a formula. Make the target hard but reachable with a redesign.
- **Best known implementation:** When another library, server, or tool in the ecosystem is faster or smaller, its value is a target the project knows is possible.
- **Stricter than the domain:** Domain thresholds describe the median of the field. Lighthouse's "good" Largest Contentful Paint of 2.5 s fits a heavy page on a slow phone, so a small static site targets about 1 s.
- **Zero for defects:** Counts of defects, such as lint warnings, panic sites, and type errors, target zero. Checks always score the share that passes, so every test and audit must pass.
- **Visible progress:** Set `poor`, the value that scores 50, near the current value when the target is far away. Each step toward the target then raises the score, instead of a score that stays near 0 until the last step.

```yaml
# uphill.yaml: the parser takes 1.2 ms now, so `poor` keeps each improvement visible on the way to
# the target in the baseline file.
speed:
  run: bun .agents/metrics/speed.ts
  # Median time to parse each fixture, measured by the benchmark harness. The target is a tenth
  # of the time the parser took on 2026-11-25, near the time to read the bytes once.
  parse-time: { poor: 1ms }
```

```yaml
# uphill.baseline.yaml: the target, and later the accepted values beside it.
speed:
  parse-time:
    "*": { target: 120µs }
```

## Depth and Detail

One number for a whole project hides where it is slow, large, or wrong. Write enough detail that a reader knows which item misses, by how much, why, and what to change first.

- **Items, not totals:** Write one line for each entry point, page, endpoint, command, source file, test, or audit. Uphill scores the p90 and lists the worst items first.
- **Several axes for each surface:** Measure time (median and tail), memory (peak resident size, allocations), I/O (bytes, requests, system calls), size, and correctness. A faster function that allocates 10 times more is not a win.
- **Input sizes:** Measure each operation at several input sizes, such as `parse 1kB`, `parse 1MB`, and `parse 100MB`, as separate items. Quadratic behavior shows only at the large size.
- **Subjects:** Use a subject, such as `node:` and `bun:`, for each runtime, build, or harness that runs the same metrics.
- **Contextual targets:** If one item needs a different target, have the script write that target on its line. Put the reason in a comment beside that code. A root entry point that bundles every subpath is such an item.
- **Notes from tools:** If an item misses, write the reason from the tool as its note. Such reasons include an assertion message, a lint rule, and an audit finding.
- **Tripwires:** Record counts that have no good value, such as exports, public types, dependencies, and `unsafe` blocks, without a target. The baseline records each count, so a change shows in review.
- **Differential checks:** When a reference implementation or a specification exists, write one check for each case where both must agree. Each disagreement is a gap to close, with the difference as its note.

```text
parse-time "1kB" 4.1µs
parse-time "1MB" 3.9ms
parse-time "100MB" 2.1s target 400ms
allocations "1MB" 18204
decode-matches-spec "vector 31" fail note "expected 0x01ff, got 0x01fe"
lint src/parse.rs fail note "line 88: index without a bounds check"
```

## Patterns

Research the published patterns of the project's language and domain before choosing metrics. The references below are examples of what to look for, not metrics to copy. Find the references that fit the project, then derive its metrics from the rules and thresholds they publish.

- **TigerStyle:** A language-neutral style for safety and performance. Its explicit limits, assertions, and static allocation show how a style rule becomes a count.
- **NASA's Power of Ten:** Rules for safety-critical code. Each rule is narrow enough that a tool can check it.
- **The language's strictest rule set:** The strictest configuration of the language's linter and type checker, and the project's AGENTS.md. Its escape hatches show where the project gives up a guarantee.
- **Lighthouse and Core Web Vitals:** How the web scores pages. Each metric has a weight and two control points, which map to `weight`, `target`, and `poor`.
- **RED and USE:** How services and resources are monitored. RED measures the rate, errors, and duration of requests. USE measures the utilization, saturation, and errors of resources.
- **Deterministic simulation and differential testing:** How correctness becomes countable. Each seed or case that fails is one check.

TigerStyle's performance rule is to sketch the four resources at design time, when the 1000 times wins are available. Use those sketches as targets, and measure each resource that the sketch names.

## Mistakes

- **A target at the current value:** The project scores 90 on the first run and has nowhere to climb.
- **One total:** A total hides the one item that regressed behind items that improved.
- **Time only:** A metric set without memory, I/O, and size accepts a speedup that costs more than it saves.
- **A metric that cannot fail:** A check that always passes, or a count that no change moves, adds noise to its category.
- **A score of zero for a failure:** A script that cannot measure exits with a non-zero status. Uphill then reports the failure, not a value.
