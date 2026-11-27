# Adapted from wevm/ondine and wevm/zyzz

Follow the [Uphill rules](.agents/uphill/rules.md), which `uphill rules --write` rewrites from each metric's description: @.agents/uphill/rules.md

# Agent Guidelines

## Project Principles

- Uphill is continuous metrics and hill climbing. Each metric has one `target`, a requirement the project must meet. A project's scripts measure it, and Uphill scores each metric against its target, compares revisions, and writes reports, so each run shows whether the project climbed toward its targets or slid back. The contract is processes and text files, so any project in any language can take part.
- Uphill measures nothing itself. Every number comes from a project script. Scripts run the tools a project already uses, such as Lighthouse, test runners, type checkers, linters, formatters, and benchmark harnesses, and turn their output into metric lines. A script measures a value itself only when no tool reports it. Built-in measures come later, only after the same script recurs across projects, and scripts remain the escape hatch.
- Keep the contract public and small. `uphill.yaml` or `uphill.toml`, at the project root or in `.agents/`, declares categories, each with a `run` command and its metrics, and categories with the same `run` share one execution. A script appends `[subject:] metric [item] value [target <value>] [note <text>]` lines to the file at `$UPHILL_METRICS`, and a non-zero exit status marks the run as failed. `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration holds each metric's targets, and `uphill --update` writes the accepted sizes, counts, and checks beside them. A change to the contract needs a decision in `.agents/plan.md`.
- Score like Lighthouse. Port `getLogNormalScore` so that a value at `target` scores 90 and a value twice the target scores 50 (half the target when higher is better), and use Lighthouse's bands: 90 to 100 good, 50 to 89 needs work, 0 to 49 poor. A metric with items collapses to its p90 unless configured. A category is the weighted mean of its metric scores, and the project score is the mean of its category scores.
- A failed or missing metric has no score, not a score of zero. Reports name the failure and keep the end of the script's stderr.
- Report change separately from score. Exact units (sizes, counts, checks) report any change. Timing changes need repeated, interleaved base and head runs. Per-item changes always show, even when the collapsed value does not move. Never gate on one timing sample.
- Keep the `engine` crate pure and deterministic: the same configuration and metric lines produce the same scores and report bytes. The `cli` crate owns processes, files, Git, and the clock.
- Write reports as self-contained files: JSON, Markdown, and HTML with inline styles and scripts. Reports make no network requests.
- Keep modules small and extensible through explicit data and narrow functions. Avoid global registration, mutable singletons, and general plugin frameworks.

## TypeScript Conventions

These apply to scripts, examples, the npm package, and the GitHub Action as they are added. The typescript pack's rules in `.agents/uphill/rules.md` hold the rest, such as compiler options, `any`, enums, imports, exports, option bags, parameters, ternaries, and TSDoc.

- Include `| undefined` when an optional property can explicitly receive `undefined`, and narrow indexed reads before use.
- Use `interface` only when declaration merging or an external ambient contract requires it.
- Author every script in TypeScript or shell, never Python. When a script needs an interpreter, write TypeScript and run it with Bun (`bun script.ts`), so scripts need no build step.
- Named imports are fine for leaf helpers, command handlers, and third-party APIs that are not module namespaces.
- Keep error classes in the module that throws them, ordered alphabetically with its public exports.
- Keep an uppercase constant only when it mirrors an external protocol or established neighboring code.
- Use `const` generic parameters when callers should retain literal or tuple types.
- Keep a sibling exported type, beside a function's namespace types, only when several functions share the domain type.
- Do not extract a named type until it is reused or makes a difficult shape materially easier to read.
- Keep shared domain types beside the module that owns the concept.
- Group a module's Zod schemas in a `schema` namespace, and derive their types beside them (`schema.Document`). Export the namespace only when another module validates against the same shape.
- Let declared return types constrain intermediate expressions. Avoid redundant local annotations.
- Keep simple two-way ternaries when both branches are easy to scan. The IIFE rule for nested ternaries applies to executable code, not TypeScript conditional types.
- When normalizing one field, read `options.field` directly instead of creating a second name.
- Declare component props as `Component.Props` inside the component's matching namespace, including nested components such as `Card.Group.Props`. Do not use inline props types or standalone component props aliases.
- Prefer short names whose meaning is clear from local context, such as `options`, `client`, `entry`, and `fn`.
- Keep wire formats, ordered tuples, protocol fields, and other order-sensitive shapes explicit. Do not alphabetize data whose order has meaning.
- Separate distinct contexts and logical steps with one blank line in every maintained module, including source, adapters, scripts, benchmarks, and test fixtures. Group setup, guard clauses, derivation, mutation or emission, and final returns so each step is easy to scan.
- Keep a lookup with its immediate guard, related local declarations, and consecutive assertions together. Separate independent branches, loops, local helper functions, and the work following a completed control-flow block. Avoid both dense uninterrupted logic and a blank line after every statement.
- Separate test setup, execution, and assertions. Keep each assertion group beside the operation it verifies. Preserve spacing inside generated source strings, snapshots, and other whitespace-sensitive fixtures.
- Audit logical spacing when adding or editing a module, since formatting tools do not infer context boundaries. Review the surrounding function as well as the changed lines, and keep spacing-only edits free of behavior changes.
- Comment invariants and non-obvious reasons, not line-by-line mechanics. Keep comments independent of plans, task IDs, and prior versions.

## Rust Conventions

Uphill is written in Rust. These conventions draw on zyzz, Ondine, oxc, ruff, and biome. Project principles, TypeScript conventions, and testing rules still apply wherever they have a Rust equivalent. The rust pack's rules in `.agents/uphill/rules.md` hold the rest, such as lints, panics, output, formatting, documentation, tests, and manifests.

### Crates and Modules

- Keep the `engine` crate pure. It reads no files, environment, clock, or process state, runs no processes, and spawns no threads. `crates/engine/clippy.toml` bans `std::fs`, `std::env`, `std::process`, `std::thread::spawn`, `std::time`, and the standard hash collections with `disallowed-methods` and `disallowed-types`.
- Keep the `cli` crate a thin adapter. It parses arguments, runs scripts, reads and writes files, creates Git worktrees, and passes data to the engine. Logic in the adapter that does not touch the environment is a defect and belongs in the engine.
- Give each concern one engine module: the configuration, metric lines, units, scoring, comparison, and one module per report format. Share number formatting from one module.
- Add no dependency without a stated reason in the commit message. Prefer hand-written code over a dependency for small parsers and formatters.

### Language and Lints

- Configure `[workspace.lints]` with `clippy::all` and `clippy::pedantic` at `warn` (priority `-1`), plus `allow_attributes`, `dbg_macro`, `get_unwrap`, `iter_over_hash_type`, `print_stderr`, `print_stdout`, `todo`, `undocumented_unsafe_blocks`, and `unimplemented`. CI runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
- If a later measure needs `unsafe`, such as `wait4` for peak memory, it gets its own module and a decision in the plan.
- Debug assertions document invariants but never replace release-mode checks.
- Use plain data, functions, and enums. Traits exist for real polymorphism, not for namespacing.
- Order `use` declarations, struct fields, and enum variants alphabetically unless order is observable. Serialization order and derived `Ord` are observable, so mark those with a comment.
- Put error construction behind `#[cold]` functions.

### Data and Determinism

- Keep categories and metrics in the order `uphill.yaml` declares them. Keep items in first-seen order, and sort them only where a report ranks them, with ties broken by first-seen order.
- Borrow input text as `&str` until output. Move values with `std::mem::take` instead of cloning.

### Numbers and Units

- Parse a unit once, at the boundary, into a base unit: bytes for sizes, nanoseconds for time, plain counts, and pass or fail for checks. Size units are decimal, so 1 kB is 1,000 B.
- Store measured values as `f64` in base units. Round only for display.
- Format every number that reaches a report through one formatter, with a fixed precision per unit and no negative zero. Never format an `f64` with `{}` in a report.
- Port Lighthouse's `getLogNormalScore`, including its `erf` approximation (Abramowitz and Stegun 7.1.26), so scores match Lighthouse and zepto's `bench/lighthouse.zig` for the same inputs. `target` takes the place of Lighthouse's p10 control point, and twice the target takes the place of its median.

### Parsing and Errors

- Parse metric lines with a hand-written parser. Report each error with its file, line, and column.
- Parse `uphill.yaml` and `uphill.toml` into the same typed structures, and reject unknown keys.
- Return errors as typed values with a code, a message, and a location. Error messages are public contract, so tests snapshot them.

### Processes

- Run each script through `sh -c` in its own process group, with the measured checkout as the working directory and a fresh `$UPHILL_METRICS` file for each run. On timeout, kill the process group.
- Treat a script's standard output and standard error as logs. Keep the end of standard error for a failed run.
- Never change the user's checkout. Measure other revisions in temporary Git worktrees, and remove them after success or failure.

### Rust Testing

- Rust tests exercise public crate functions or the built binary end to end.
- Run CLI tests against the built binary (`env!("CARGO_BIN_EXE_uphill")`) with fixture projects in temporary directories, real shell scripts, and real Git repositories.
- Derive expected scores independently of the implementation, from Lighthouse's published guarantees (a value at `target` scores 0.9 and a value at twice the target scores 0.5) and from values computed by Lighthouse's own JavaScript.

## Alphabetical Ordering

- Alphabetize imports, named import/export specifiers, public exports, type/interface properties, object properties, enum/union members, unordered lists, configuration maps, scripts, and dependencies. Use case-insensitive lexical order consistently.
- Keep attached documentation with the declaration or property it describes. Keep function overloads and their namespace together. Order local declarations alphabetically within dependency-compatible groups. Do not introduce use-before-initialization or reorder execution.
- Preserve order with observable semantics: categories and metrics in `uphill.yaml`, metric lines as scripts write them, report sections, tuples, package file inclusions followed by exclusions, workflow steps, and regression fixtures testing those orders. Mark intentional exceptions in nearby documentation or comments.
- Keep `name` and `on` first in GitHub workflows, and alphabetize the remaining top-level keys. Preserve the scaffold package.json group order around `[!start-pkg]`, and alphabetize entries within each group. Do not alphabetize prose sections or sequential implementation phases mechanically.

## Module and Instance Conventions

- Organize each module file as one conceptual namespace containing its public types, constants, functions, and errors. Consumers should read calls as `Module.operation(...)`.
- Prefer stateless module functions for pure behavior. Do not create an instance when inputs fully describe the operation.
- When behavior needs dependencies or lifecycle state, expose a factory such as `Module.create(options)` or `Module.runtime(options)` that returns a plain object of operations and data.
- Construct instances explicitly at the application boundary and pass them down. Do not hide construction in imports or module-scope singletons.
- Scope an instance to the lifecycle that owns its state, such as one CLI run, Worker isolate, request pipeline, or test. Do not share mutable state more broadly than required.
- Inject environment-specific capabilities through factory options or narrow structural types. Keep filesystem, network, cache, clock, and platform bindings out of domain modules.
- Type dependencies by the smallest capability the module consumes, not by the concrete SDK client. This keeps adapters interchangeable without wrapper classes.
- Let factory operations close over shared dependencies and state. Do not return methods that depend on `this` or require binding.
- Name the returned public shape after its role (`Runtime`, `Client`, `Cache`, `Store`) when that role is meaningful. Use `create.ReturnType` for a factory-specific shape that has no independent domain name.
- Keep one authoritative instance value. Derive related helpers and views from that instance instead of duplicating configuration or state.
- Keep mutable caches and registries private to the instance. Expose explicit operations, and provide reset or disposal only when the lifecycle requires it.
- Avoid side-effect registration. `sideEffects: false` means an import used only to install global behavior can disappear from a bundle.
- Keep default implementations directly reachable from the factory or resolver that selects them so bundlers can tree-shake unused paths.
- Pass an existing receiver first to stateless operations (`Github.publish(client, options)`). Create a factory only when several operations genuinely share dependencies, state, or lifecycle.
- Keep transport-independent planning, parsing, and normalization pure. Put filesystem, GitHub, and Worker behavior in thin adapters around that core.
- Keep internal helpers under `internal/` and export them only when another module has a real contract with them.
- Add new public modules through the owning entrypoint as documented namespace exports. Keep the public surface lean and derive values that the library already knows.
- A framework-mandated class should delegate reusable logic to module functions so the class remains a small lifecycle adapter.

## Type Inference Conventions

- Preserve literal inputs through public helpers when those literals affect the output type.
- Keep generic types flowing from inputs through callbacks and return values. Do not erase them to `any` at an internal seam.
- Prevent public callbacks, options, and return values from leaking `any`.
- Add consumer `.test-d.ts` fixtures beside their owning module (for example, `src/Module.test-d.ts`) with `expectTypeOf` and expected compiler errors when public inference or narrowing changes. Import public entrypoints, and make sure TypeScript actually checks the fixtures.
- Revisit inference after changing an API. Prefer a narrower useful contract over a broad type that merely compiles.

## Abstraction Conventions

- Start with concrete code and extract only after repeated uses reveal a stable shared contract.
- Prefer small local duplication over an abstraction that adds flags, modes, or call-site-specific branches.
- Wait for at least three concrete uses before introducing a general abstraction unless a hard boundary already exists.
- Optimize for code that is easy to change, not maximum DRYness.
- Keep authoritative state, configuration, schemas, and constants in one place, and derive dependent values.
- Avoid wrappers that only rename another function or mirror an SDK without narrowing capabilities or adding a domain contract.

## Code Documentation

- Place a TypeScript module's TSDoc `@module` comment before imports, or after a required shebang. Module comments cover entrypoints, internal modules, tooling, fixtures, integration tests, type fixtures, and benchmarks. Describe the module rather than repeating its filename.
- Write or update the contract documentation alongside the implementation.
- Document caller-visible purpose, inputs, output, defaults, errors, and side effects. Keep low-level wiring in nearby implementation comments.
- Update the owning entrypoint documentation when adding or changing a public module.

## Writing Documentation

Applies to `docs/`, the README, and other Markdown documentation. Prose Conventions also apply, and the writing-docs pack's rules in `.agents/uphill/rules.md` set the limits on sentences, paragraphs, and headings.

### Voice and Terms

- Write for developers who maintain a project and already run tests, benchmarks, or size checks. Use Lighthouse's terms, such as score, metric, and category, without redefining them.
- Use Uphill's own terms consistently: script, metric line, item, target, round, and comparison. Define each one once, on the page that introduces it.
- Explain behavior through `uphill.yaml`, the metric lines a script writes, and the report that results. Leave engine internals to the pages that document them.
- Keep guides language-neutral. When setup differs by language or ecosystem, show every one, such as in `####` subsections for shell, TypeScript, and Rust, rather than assuming one.
- Do not state the obvious. When a feature differs by platform, state that limit directly.
- Do not mention other measurement tools in guides. Name them only on a Comparisons page and in migration guides.

### Paragraphs and Sections

- Keep each paragraph to one idea. Tighten wording rather than mechanically splitting paragraphs.
- Write at most three paragraphs per section. When a topic needs more, split it into sections with their own headings.
- Prefer an example in every section, such as an `uphill.yaml` excerpt, the metric lines a script writes, or the report output. Show the behavior rather than only describing it or deferring to another page.
- Use bullets for enumerable content and tables for comparisons.
- Start each item of a topic list with a bold label that ends in a colon, such as ``- **`target`:** The value that scores 90.``
- Mark preview or unimplemented features with GitHub Markdown `> [!NOTE]` callouts scoped to the affected content. Do not add "Available" labels or status suffixes to headings. Examples must preserve the documented contract and must not imply preview features already work.

### Headings

- Keep headings short and in Title Case so they fit the documentation index and the outline GitHub shows. Product names keep their own casing.

### Guide Structure

- Organize each guide into these `##` sections, in order:
  - **Overview:** Introduces the feature in at most three paragraphs and ends with a basic example. Use one code block, or one subsection per file or language when the example spans several.
  - **Walkthrough:** Builds one complete example step by step, from `uphill.yaml` and a script to the report.
  - **Recipes:** Lists focused tasks the feature supports, each under its own `###` heading.
  - **More:** Links to related guides and reference pages, one per line, each with a short description.
- When a guide has no recipes, hoist its sections to `##` headings between Overview and More.
- Group guides by topic, and preserve recipe coverage when consolidating pages.
- Write migration guides as one `##` section per pattern, each with a before-and-after pair: the other tool's setup first, then the Uphill equivalent. Highlight and comment the corresponding lines in both.

### Navigation

- Order documentation by common tasks: getting started, writing scripts, scoring, and comparing before reference pages. Preserve this reading order instead of alphabetizing navigation.
- Maintain three groups in `docs/README.md`: Introduction, Guides, and Reference. Reference covers `uphill.yaml` keys, the metric line format, units, scoring, report formats, and CLI commands.
- Give each configuration key, CLI flag, and report field its own subheading, type, description, and small usage snippet. Document defaults where applicable.
- Link pages with relative paths, such as `../reference/scoring.md#curve`, so links work on GitHub and in a checkout. Link to `docs/` pages from Markdown outside it.

### Code Examples

- Keep examples small and focused on the documented behavior. Prefer a snippet, concrete example, or useful visual to a long conceptual explanation, and avoid repeating what the example already shows.
- Give every documentation code block at least one comment explaining the code, including configuration and output examples. Formats without comments, such as JSON and plain metric lines, are exempt. Write short comments on their own line above the code they describe. State what the code does or why, rather than restating it.
- Number the comments (`# 1.`, `// 1.`) when an example follows ordered steps, such as configure, measure, and report.
- Keep `[!code hl]` markers separate from explanatory comments. When highlighted code has a comment beside or above it, highlight the comment too.

## Prose Conventions

Applies to documentation, comments, TSDoc, interface copy, commit messages, and pull requests. Adapted from the Prose Conventions of `tempoxyz/id` through Ondine. The writing-docs pack's rules hold the rest, such as voice, word choice, verb tenses, punctuation, and emoji.

- Second person is valid only in interface copy.
- Describe behavior in technical terms rather than by the experience it produces. Prefer `scores the p90 of 1,912 items` over `gives a feel for typical size`.
- Write formally, concisely, and technically.
- State an invariant or a reason the code cannot show on its own. Leave out justification the code already makes plain.
- Write close to ASD-STE100 Simplified Technical English. Apply its writing rules below, but not its full dictionary.
- Use the short, common word: "use", "start", "before", "about", "make sure", not "utilize", "commence", "prior to", "approximately", "ensure". Technical names and Uphill terms, such as "target", "item", and "round", are always valid.
- Use one word for one thing, and give each word one meaning and one part of speech. Do not use a synonym for variety.
- Keep an instruction to 20 words or fewer. Give one instruction in each sentence, unless the actions occur at the same time.
- Keep each paragraph to one topic. Use a vertical list for sequential steps or several conditions.
- Put a condition before the instruction it controls: "If a script fails, read the end of its stderr in the report."
- Use the passive voice in descriptions only when the agent is unknown or not important.
- An `-ing` word in a technical name is valid, though the main verb is never a progressive form.
- Do not stack more than three nouns, except in a technical name. Break a longer cluster with a preposition.
- Do not leave out "the", "a", or "this" to make a sentence shorter.
- Vary sentence construction. One shape repeated across a file, such as an assertion followed by a colon and its reason, reads as a writing style rather than as information.
- Use a diagram, not prose, to show a flow or how components connect, and keep it to about 12 nodes. Use a table, not a paragraph, to compare options or map inputs to outputs.
- Keep PR titles and bodies short and concrete. Explain the problem and resulting behavior in a few sentences, include one small example, and add one validation line. Use a usage snippet, before/after behavior, or relevant command. Link detailed reports.

## Testing Conventions

- Runtime coverage is integration-only. Do not write unit tests, private-helper tests, or per-function suites disguised as integration tests.
- No mocking, stubbing, fake implementations, module replacements, fake timers, or stubbed globals. Use real processes, shell scripts, temporary directories, Git repositories, and browsers. Fixture projects and deterministic input data are allowed, but replacement implementations are not.
- Keep reusable fixture projects under each crate's `tests/fixtures/`. Each fixture is a small project with an `uphill.yaml` and the scripts it runs.
- Cover complete supported flows as they land: configuration to script runs, metric lines to scores, scores to reports, and two revisions to a comparison. Before a later stage exists, test the real available public boundary, and do not fabricate a downstream stage.
- Snapshot values and results individually. Do not assemble unrelated values into one structure solely to combine snapshot assertions. Snapshot a structure directly when it is the actual result under test.
- Assert observable results and public diagnostics with inline snapshots. Mask only genuinely nondeterministic fields, such as temporary paths and durations, and keep deterministic values exact. Review generated snapshots before accepting them. Never derive expected output from the implementation under test.
- Verify the HTML report in a real browser at desktop and phone widths, in light and dark schemes. A markup snapshot alone is not rendering proof.
- Add an integration regression scenario for every bug fix. Track coverage of consumer workflows and error paths rather than targeting a test count or percentage.
- Use bounded waits for observable conditions, isolate real resources, and clean them up after success or failure. Do not hide flakes with arbitrary sleeps or retries.

## Benchmark Conventions

- Once the runner works, Uphill measures itself with its own `.agents/uphill/uphill.yaml`: binary size, and the time to run and report the fixture projects.
- Confirm timing regressions with repeated, interleaved samples on one machine, never with a single run.
- Keep generated reports under the ignored `.uphill/` directory. Numbers committed to documentation name one run, commit, machine, and sample count, and keep every result including losses.

## Workflow Conventions

- Use the smallest command that covers the changed behavior. Run focused tests while iterating, such as `cargo test -p uphill`.
- Run `cargo clippy --workspace --all-targets -- -D warnings` after Rust changes, and `pnpm check:types` after TypeScript changes once TypeScript packages exist.
- Run `cargo fmt --all` before committing. Inspect and keep only task-related changes.
- Run `git diff --check` and inspect the final diff before reporting completion.

## Git Conventions

- Commit types: `feat:`, `fix:`, `perf:`, `refactor:`, `docs:`, `test:`, `chore:`. Scope optional. The git pack's rules hold the rest of the commit message format.
- Name the thing that changed, in the identifiers the codebase already uses. Prefer `feat: collapse list metrics to p90 in engine::score` over `feat: make big lists easier to score`.
- Write each message to stand alone for a reader who has not seen the pull request or the task that produced it.
- Never force push and never amend a pushed commit. Correct a mistake with a follow-up commit.
- Always commit and push directly to `main`. Never create or push to another branch, and never open a pull request.

## UI Conventions

- The HTML report is one file with inline CSS and JavaScript and no network requests. It follows Lighthouse's report layout: a gauge per category, score bands, a grid of metrics with each value against its target, and a collapsible row per list metric with its items worst first.
- Support light and dark schemes through `prefers-color-scheme`, and respect reduced-motion preferences.
- Use accessible native controls. Implement keyboard activation, visible focus, disabled behavior, and accessible names alongside each control.
- The HTML report embeds its own stylesheet, because it ships inside the binary as one file.

## Repository Layout

- `crates/engine/`: the pure engine. `crates/cli/`: the `uphill` package, which builds the `uphill` binary, and its integration tests.
- `examples/`: small projects in different languages, each with an `uphill.yaml` and the scripts it runs. CLI tests run `uphill check` on each one.
- `packs/`: packs that measure common conventions, each an `uphill.yaml`, its scripts, and a `README.md` that `uphill add` prints. A script runs the tools that already check a rule, and measures a rule itself only when no tool does.
- `SKILL.md`: the `uphill` skill, which `uphill init` embeds at the end of its brief.
- `.agents/uphill/`: Uphill measuring itself, with `uphill.yaml`, its baseline file, the scripts in `metrics/`, and the generated `rules.md`.
- `.agents/plan.md` tracks phases, decisions, and acceptance gates.
- `npm/`: the `uphill` npm package, a pnpm workspace package built with zile from the zile template. `npm/src/bin.ts` runs the binary for the machine from `npm/binaries/<platform>-<arch>/`, which the Main workflow fills from the release archives. Its version is the release version, and `npm/version.ts` copies it into `Cargo.toml` and `Cargo.lock`. Changesets reads `.changeset/` at the repository root.
- `install.sh`, `.github/workflows/main.yml`, and `.github/workflows/binaries.yml`: the install script, and the release. A change that users see adds a changeset with `pnpm changeset`. On `main`, the Main workflow keeps a `chore: version packages` pull request open, and merging it builds the archives with `binaries.yml`, creates the `v<version>` GitHub Release, and publishes `uphill` to npm. The changesets bot owns that branch and pull request.
- `action.yml`: the GitHub Action, which runs `uphill --compare` on a pull request and keeps one comment with the report. `assets/gauges/` holds the score gauges the comment links, which `bun assets/gauges.ts` writes.
- `docs/`: the documentation, one Markdown page per topic, which GitHub renders. `docs/README.md` lists the pages in reading order.

## Tooling

- Declare TypeScript dependencies with `catalog:` references in `pnpm-workspace.yaml`, format and lint with Vite Plus (oxfmt and oxlint), build packages with zile, and version them with Changesets. Prettier is banned.

## Commands

- `cargo run -p uphill -- --help` runs the `uphill` binary from source.
- `cargo test --workspace` runs every integration test.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` check formatting and lints.
- `cargo build --release` builds the release binary at `target/release/uphill`.
- `cargo run -p uphill -- check` runs Uphill on itself with `.agents/uphill/uphill.yaml`.
- `pnpm install` installs the TypeScript tooling. `pnpm check` formats and lints TypeScript with Vite Plus, and `pnpm check:types` checks types.
- `pnpm --filter uphill build` builds the npm launcher with zile. `pnpm changeset` writes a changeset, a release note for the next version.
