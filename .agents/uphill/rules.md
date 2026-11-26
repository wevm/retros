# Rules

Uphill writes these rules from each metric's `description`. Write code that meets them. `uphill` scores each metric against its target, and `uphill --compare HEAD --compact` lists what a change made worse.

## Footprint

- `binary-size`: At most 1 MB.
- `dependencies`: At most 10.
- `entry-size`: Keep the gzipped bundle small for a consumer that imports one export. Do not pull unrelated modules or packages into an export. At most 10 kB.
- `tree-shaking`: Make each entry point bundle to nothing when unused. Annotate top-level calls with `/*#__PURE__*/`, and avoid runtime TypeScript namespaces and static class fields.
- `hot-entry-size`: Keep the gzipped bundle of each hot item especially small. At most 10 kB.

## Speed

- `startup`: At most 1 ms.
- `check`: At most 5 ms.
- `call-time`: Keep each public function fast per call, with cold and warm caches. At most 20 µs.
- `call-memory`: Keep the heap bytes that each call of a public function allocates small. At most 20 kB.
- `import-time`: Keep each entry point fast to import. Avoid work at module scope. At most 60 ms.
- `import-memory`: Keep the heap that each entry point holds once imported small. At most 3.5 MB.
- `hot-call-time`: Keep each hot function especially fast per call. At most 20 µs.
- `hot-call-memory`: Keep the heap bytes that each call of a hot function allocates especially small. At most 20 kB.
- `hot-import-time`: Keep each hot entry point especially fast to import. At most 60 ms.
- `hot-import-memory`: Keep the heap that each hot entry point holds once imported especially small. At most 3.5 MB.

## Correctness

- `clippy`: Fix every finding of clippy's default and pedantic lints.
- `unfinished`: Finish each function. Leave no `todo!` or `unimplemented!` in the code.
- `panic-sites`: Encode invariants in types instead of `unwrap`, `expect`, slicing, or `panic!`. Use `get`, pattern matching, and `let` chains.
- `allow-attributes`: Suppress a lint with `#[expect(lint, reason = "...")]`, never `#[allow]`.
- `unsafe-code`: Write no `unsafe` code. If an exception is necessary, document each block with `// SAFETY:`.
- `inline-always`: Mark `#[inline(always)]` only with a measured reason in an `#[expect(clippy::inline_always, reason = "...")]`.
- `hash-iteration`: Never iterate a hash map to produce output. Use a `Vec`, a `BTreeMap`, or the declared order.
- `shared-mutability`: Do not build `Rc<RefCell<_>>` graphs or wrap mutexes and buffers in `Rc`.
- `workspace-lints`: Set clippy's `all` and `pedantic` groups and the restriction lints the conventions list in `[workspace.lints]`.
- `allowed-lints`: Allow only `module_name_repetitions`, `must_use_candidate`, `similar_names`, and `too_many_lines` workspace-wide.
- `unsafe-denied`: Deny `unsafe_code` in the workspace lints.
- `swallowed-errors`: Keep the original error. Do not discard it with `map_err(|_| ...)`.
- `redundant-clones`: Do not clone a value that is not used again (clippy `redundant_clone`). Move it instead.
- `string-errors`: Return errors as typed values, not `String`, `&str`, or `Box<dyn Error>`.
- `unbounded-channels`: Give each channel a capacity.
- `type-errors`: Fix every type error. Run the project's `tsc` or `tsgo` to check.
- `compiler-options`: Set `strict`, `exactOptionalPropertyTypes`, `noUncheckedIndexedAccess`, and `verbatimModuleSyntax` in tsconfig.json. Treat them as design constraints.
- `explicit-any`: Avoid `any`. Use a precise type, validation, or narrowing (oxlint `typescript/no-explicit-any`).
- `lint`: Fix every report of the project's linter with its own configuration.
- `math-random`: Use `crypto.getRandomValues()`, not `Math.random()` (oxlint `eslint/no-restricted-properties`).
- `console-calls`: Do not log through `console` in files under `src/` (oxlint `eslint/no-console`).

## Style

- `formatted`: Format each Rust file with rustfmt (`cargo fmt`). Format each TypeScript file with the project's formatter and its own configuration.
- `print-macros`: Write output through `std::io::Write`, not `println!`, `eprintln!`, or `dbg!`.
- `comment-dividers`: Do not write decorative divider comments, such as `// -----`.
- `duration-names`: Name the unit in each `Duration::from_*` argument, such as `timeout_secs`.
- `import-renames`: Keep the item's own name in `use` declarations. Do not rename it with `as`.
- `log-interpolation`: Record values in log macros as fields, not interpolated into the message.
- `wide-tuples`: Use a struct instead of a tuple of three or more types.
- `whitespace`: Leave no trailing whitespace, blank lines at the end, or conflict markers in a tracked text file (`git diff --check`).
- `workflow-keys`: Start each GitHub workflow with `name` and `on`, then list its other top-level keys in alphabetical order.
- `enums`: Avoid enums. Use unions or `as const` objects.
- `interfaces`: Declare shapes with `type`, not `interface` (oxlint `typescript/consistent-type-definitions`).
- `mutable-arrays`: Annotate arrays as `readonly T[]`. Keep an array mutable only where code mutates the binding or a function's result.
- `import-extensions`: Name the file extension in each import, so the source stays valid under NodeNext (oxlint `import/extensions`).
- `type-imports`: Use `import type` and `export type` for type-only imports and exports.
- `uppercase-constants`: Name module constants in camelCase, not UPPER_CASE.
- `optional-options`: Default option bags in the signature (`options: fn.Options = {}`), not with `options?`.
- `option-names`: Name option bags `options`, not `args`, `opts`, or `params`.
- `positional-parameters`: Give each function at most two positional parameters, a receiver and then an options bag (oxlint `eslint/max-params`).
- `destructured-parameters`: Do not destructure parameters in the signature. Take each parameter whole and destructure it in the body.
- `nested-ternaries`: Avoid nested ternaries. Use an IIFE with early returns (oxlint `eslint/no-nested-ternary`).
- `inline-object-types`: Do not annotate local variables with inline object types. Name the type above the use.
- `function-namespaces`: Put a function's types in its namespace (`create.Options`), not beside it (`CreateOptions`).
- `immediate-returns`: Return values directly. Do not bind a value only to return it on the next line.
- `let-across-try`: Derive a fallible value in an IIFE, not in a `let` assigned across `try` and `catch`.
- `section-dividers`: Do not write section-divider comments, such as `// ----`.

## Architecture

- `unreachable-pub`: Mark items that another crate cannot reach `pub(crate)` (rustc `unreachable_pub`).
- `orphaned-files`: Make sure a workspace target, compiler input, or rustfmt traversal reaches each Rust file. Delete files that nothing reaches.
- `import-cycles`: Avoid import cycles. No module may import itself through other modules (oxlint `import/no-cycle`).
- `namespace-imports`: Import module-shaped files, such as `./Store.js`, as namespaces (`import * as Store`). Types and components may use named imports.
- `node-imports`: Import Node built-ins as namespaces (`import * as fs from 'node:fs'`).
- `flat-reexports`: Re-export modules as namespaces (`export * as Store`). Do not flatten them into a barrel with `export *` or `export { … } from`.
- `dynamic-imports`: Use static imports. Use a dynamic import only at a real runtime or bundle boundary.
- `side-effect-imports`: Do not import modules for their side effects, such as `import './register.js'` (oxlint `import/no-unassigned-import`).
- `classes`: Use functions and plain data. Limit classes to errors and framework entrypoints.
- `error-names`: Set a namespaced `name` on each error class, such as `'Config.InvalidError'`.
- `this-outside-classes`: Use `this` only in classes. Make factories close over their state.
- `module-state`: Keep only constants at module scope. Do not use a module-scope `let` or an empty `Map` or `Set` cache.

## Documentation

- `module-docs`: Start each Rust file with a `//!` comment that describes the module's responsibility. Start each TypeScript file with a TSDoc `@module` comment that describes its responsibility.
- `missing-docs`: Document every public item (rustc `missing_docs`).
- `rustdoc`: Make sure `cargo doc` builds without warnings.
- `sentence-words`: Keep each sentence in a paragraph to 25 words or fewer.
- `paragraph-words`: Keep each paragraph to about 40 words, and never more than 50.
- `paragraph-sentences`: Keep each paragraph to six sentences or fewer.
- `passive-voice`: Use the active voice. Avoid passive phrases such as "is processed" (write-good `passive`).
- `wordy-words`: Use the short word instead of a wordy phrase, such as "to" for "in order to" (write-good `tooWordy`).
- `weasel-words`: Avoid weasel words such as "very" (write-good `weasel`).
- `second-person`: Write about the code, not the reader. Avoid "you", "your", and "yourself".
- `conversational-verbs`: Use precise verbs such as "returns" or "reports", not get, give, hand, know, say, see, or tell.
- `verb-tenses`: Use the imperative, simple present, simple past, or simple future. Do not use a progressive or perfect form.
- `latin-abbreviations`: Write "for example" and "that is" instead of "e.g.", "i.e.", and "etc.".
- `semicolons`: Do not use semicolons in prose. Split the sentence, or use a comma, colon, or parenthesis.
- `em-dashes`: Do not use em dashes in prose. Split the sentence, or use a comma, colon, or parenthesis.
- `emoji`: Do not use emoji in prose.
- `heading-case`: Write headings in Title Case. Lowercase minor words unless they begin or end the heading.
- `heading-words`: Keep every heading to fewer than four words.
- `export-docs`: Document every export with TSDoc. Overloads share the comment on the first signature.
- `property-docs`: Document every property of an exported type with TSDoc.

## Testing

- `unit-tests`: Put tests in each crate's `tests/` directory. Write no unit tests in `src/`.
- `inline-snapshots`: Assert with inline `insta` snapshots in each Rust test file, not `assert_eq!`. Assert with inline snapshots in TypeScript tests, not with `toBe` or `toEqual`.
- `mock-crates`: Do not depend on a mocking crate, such as mockall or wiremock.
- `compound-asserts`: Check one condition in each assertion. Do not join two with `&&` or `||`.
- `test-sleeps`: Make tests wait for a condition, not for a fixed `sleep`.
- `weak-error-assertions`: Check which error a test receives, not only `is_err()`.
- `mocks`: Test with real implementations. Do not use `vi.mock`, `vi.fn`, fake timers, stubbed globals, or mocking packages (oxlint restricted-method and restricted-import rules).
- `snapshot-files`: Keep snapshots inline. Do not commit snapshot files.
- `sleeps`: Wait for observable conditions in tests, not with `setTimeout` or `Bun.sleep` (oxlint restricted-global and restricted-property rules).
- `split-test-files`: Write one test file per module. Do not split it into `Module.context.test.ts` files.

## Configuration

- `edition`: Use edition 2024 in each package.
- `inherited-lints`: Inherit the workspace's lints in each workspace member (`lints.workspace = true`).
- `shared-versions`: Take dependency versions from `[workspace.dependencies]` in each workspace member.
- `toolchain`: Pin a version in rust-toolchain.toml, not a channel such as `stable`.
- `rust-version`: Declare `rust-version` in the workspace.
- `release-profile`: Set `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, and `strip = "symbols"` in the release profile.
- `dependencies`: Add a dependency only with a stated reason. The baseline records the count without a target, so each change shows in review.
- `javascript-files`: Write source files in TypeScript, not JavaScript. Configuration files are exempt.
- `side-effects`: Declare `sideEffects` as a boolean or a file-pattern list in each named, non-private package.
- `catalog-versions`: Reference dependency versions with `catalog:` in each package.json of a pnpm workspace.
- `script-runners`: Run TypeScript scripts directly, not through `tsx` or `ts-node`.
- `sorted-keys`: Keep the dependencies and scripts of each package.json in alphabetical order.

## Commits

- `commit-type`: Start each commit header with a conventional type, such as `feat:` or `fix:`, and a subject (commitlint `type-enum`, `type-case`, `type-empty`, and `subject-empty`).
- `subject-case`: Start each subject in lowercase (commitlint `subject-case`).
- `subject-full-stop`: End each subject without a period (commitlint `subject-full-stop`).
- `header-length`: Keep each commit header to 100 characters or fewer (commitlint `header-max-length`).
- `message-body`: Put a blank line before the body and the footer, and wrap them at 100 characters (commitlint `body-leading-blank`, `body-max-line-length`, and the matching footer rules).
- `subject-reasons`: State the change in the subject, not the reason. Leave out "because", "so that", and "in order to".
- `imperative-subjects`: Open each subject with a verb in its base form, such as "add", not "adds" or "added".
- `fix-tests`: Add or edit a test source file in each `fix:` commit.
- `changeset-sentences`: Write each changeset as one sentence that opens with a past-tense verb, such as "Added".
- `tracked-output`: Do not commit build output, installed packages, or `.env` files.

## Coverage

- `benched`: Call each public function in the tests with arguments that rebuild in another process, such as plain data. Make sure the call returns or resolves.

## Types

- `instantiations`: Keep each public export cheap to type-check. Avoid types that need many instantiations to resolve. At most 1000.
- `check-time`: Keep each public export fast to type-check. At most 5 ms.
- `check-memory`: Keep the heap bytes that type-checking each public export allocates small. At most 2 MB.
- `bench-instantiations`: Keep the instantiations of each benchmarked type test low, within any count the test pins. At most 20000.
- `hot-instantiations`: Keep each hot export especially cheap to type-check. At most 1000.

## Call types

- `instantiations`: Keep each public function cheap to type-check at the call sites that the tests write. At most 1000.
- `hot-instantiations`: Keep each hot function especially cheap to type-check at its call sites. At most 1000.

## Type coverage

- `type-benched`: Call each public function in the runtime or type tests. Make sure the imports and fixtures of that call resolve.

## Fuzz

- `fuzzed`: Make sure each public function has a fuzz harness that runs.
- `fuzz-clean`: Handle every input that keeps the declared types. Throw no undocumented TypeError, RangeError, or SyntaxError, and avoid stack overflows, hangs, and runaway memory. Keep each property in `fuzz.properties` true.
- `fuzz-coverage`: Make every line of a public function's own source reachable by fuzzing. At least 90.
- `hot-fuzz-coverage`: Make sure every line of a hot item's own source runs under fuzzing. At least 100.
