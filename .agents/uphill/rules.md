# Rules

Uphill writes these rules from each metric's `description`. Write code that meets them. `uphill` scores each metric against its target, and `uphill --compare HEAD --compact` lists what a change made worse.

## Correctness

- Fix every finding of clippy's default and pedantic lints.
- Finish each function. Leave no `todo!` or `unimplemented!` in the code.
- Encode invariants in types instead of `unwrap`, `expect`, slicing, or `panic!`. Use `get`, pattern matching, and `let` chains.
- Suppress a lint with `#[expect(lint, reason = "...")]`, never `#[allow]`.
- Write no `unsafe` code. If an exception is necessary, document each block with `// SAFETY:`.
- Mark `#[inline(always)]` only with a measured reason in an `#[expect(clippy::inline_always, reason = "...")]`.
- Never iterate a hash map to produce output. Use a `Vec`, a `BTreeMap`, or the declared order.
- Do not build `Rc<RefCell<_>>` graphs or wrap mutexes and buffers in `Rc`.
- Set clippy's `all` and `pedantic` groups and the restriction lints the conventions list in `[workspace.lints]`.
- Allow only `module_name_repetitions`, `must_use_candidate`, `similar_names`, and `too_many_lines` workspace-wide.
- Deny `unsafe_code` in the workspace lints.
- Keep the original error. Do not discard it with `map_err(|_| ...)`.
- Do not clone a value that is not used again (clippy `redundant_clone`). Move it instead.
- Return errors as typed values, not `String`, `&str`, or `Box<dyn Error>`.
- Give each channel a capacity.
- Fix every type error. Run the project's `tsc` or `tsgo` to check.
- Set `strict`, `exactOptionalPropertyTypes`, `noUncheckedIndexedAccess`, and `verbatimModuleSyntax` in tsconfig.json. Treat them as design constraints.
- Avoid `any`. Use a precise type, validation, or narrowing (oxlint `typescript/no-explicit-any`).
- Fix every report of the project's linter with its own configuration.
- Use `crypto.getRandomValues()`, not `Math.random()` (oxlint `eslint/no-restricted-properties`).
- Do not log through `console` in files under `src/` (oxlint `eslint/no-console`).

## Style

- Format each Rust file with rustfmt (`cargo fmt`). Format each TypeScript file with the project's formatter and its own configuration.
- Write output through `std::io::Write`, not `println!`, `eprintln!`, or `dbg!`.
- Do not write decorative divider comments, such as `// -----`.
- Name the unit in each `Duration::from_*` argument, such as `timeout_secs`.
- Keep the item's own name in `use` declarations. Do not rename it with `as`.
- Record values in log macros as fields, not interpolated into the message.
- Use a struct instead of a tuple of three or more types.
- Leave no trailing whitespace, blank lines at the end, or conflict markers in a tracked text file (`git diff --check`).
- Start each GitHub workflow with `name` and `on`, then list its other top-level keys in alphabetical order.
- Avoid enums. Use unions or `as const` objects.
- Declare shapes with `type`, not `interface` (oxlint `typescript/consistent-type-definitions`).
- Annotate arrays as `readonly T[]`. Keep an array mutable only where code mutates the binding or a function's result.
- Name the file extension in each import, so the source stays valid under NodeNext (oxlint `import/extensions`).
- Use `import type` and `export type` for type-only imports and exports.
- Name module constants in camelCase, not UPPER_CASE.
- Default option bags in the signature (`options: fn.Options = {}`), not with `options?`.
- Name option bags `options`, not `args`, `opts`, or `params`.
- Give each function at most two positional parameters, a receiver and then an options bag (oxlint `eslint/max-params`).
- Do not destructure parameters in the signature. Take each parameter whole and destructure it in the body.
- Avoid nested ternaries. Use an IIFE with early returns (oxlint `eslint/no-nested-ternary`).
- Do not annotate local variables with inline object types. Name the type above the use.
- Put a function's types in its namespace (`create.Options`), not beside it (`CreateOptions`).
- Return values directly. Do not bind a value only to return it on the next line.
- Derive a fallible value in an IIFE, not in a `let` assigned across `try` and `catch`.
- Do not write section-divider comments, such as `// ----`.

## Architecture

- Mark items that another crate cannot reach `pub(crate)` (rustc `unreachable_pub`).
- Make sure a workspace target, compiler input, or rustfmt traversal reaches each Rust file. Delete files that nothing reaches.
- Avoid import cycles. No module may import itself through other modules (oxlint `import/no-cycle`).
- Import module-shaped files, such as `./Store.js`, as namespaces (`import * as Store`). Types and components may use named imports.
- Import Node built-ins as namespaces (`import * as fs from 'node:fs'`).
- Re-export modules as namespaces (`export * as Store`). Do not flatten them into a barrel with `export *` or `export { … } from`.
- Use static imports. Use a dynamic import only at a real runtime or bundle boundary.
- Do not import modules for their side effects, such as `import './register.js'` (oxlint `import/no-unassigned-import`).
- Use functions and plain data. Limit classes to errors and framework entrypoints.
- Set a namespaced `name` on each error class, such as `'Config.InvalidError'`.
- Use `this` only in classes. Make factories close over their state.
- Keep only constants at module scope. Do not use a module-scope `let` or an empty `Map` or `Set` cache.

## Documentation

- Start each Rust file with a `//!` comment that describes the module's responsibility. Start each TypeScript file with a TSDoc `@module` comment that describes its responsibility.
- Document every public item (rustc `missing_docs`).
- Make sure `cargo doc` builds without warnings.
- Keep each sentence in a paragraph to 25 words or fewer.
- Keep each paragraph to about 40 words, and never more than 50.
- Keep each paragraph to six sentences or fewer.
- Use the active voice. Avoid passive phrases such as "is processed" (write-good `passive`).
- Use the short word instead of a wordy phrase, such as "to" for "in order to" (write-good `tooWordy`).
- Avoid weasel words such as "very" (write-good `weasel`).
- Write about the code, not the reader. Avoid "you", "your", and "yourself".
- Use precise verbs such as "returns" or "reports", not get, give, hand, know, say, see, or tell.
- Use the imperative, simple present, simple past, or simple future. Do not use a progressive or perfect form.
- Write "for example" and "that is" instead of "e.g.", "i.e.", and "etc.".
- Do not use semicolons in prose. Split the sentence, or use a comma, colon, or parenthesis.
- Do not use em dashes in prose. Split the sentence, or use a comma, colon, or parenthesis.
- Do not use emoji in prose.
- Write headings in Title Case. Lowercase minor words unless they begin or end the heading.
- Keep every heading to fewer than four words.
- Document every export with TSDoc. Overloads share the comment on the first signature.
- Document every property of an exported type with TSDoc.

## Testing

- Put tests in each crate's `tests/` directory. Write no unit tests in `src/`.
- Assert with inline `insta` snapshots in each Rust test file, not `assert_eq!`. Assert with inline snapshots in TypeScript tests, not with `toBe` or `toEqual`.
- Do not depend on a mocking crate, such as mockall or wiremock.
- Check one condition in each assertion. Do not join two with `&&` or `||`.
- Make tests wait for a condition, not for a fixed `sleep`.
- Check which error a test receives, not only `is_err()`.
- Test with real implementations. Do not use `vi.mock`, `vi.fn`, fake timers, stubbed globals, or mocking packages (oxlint restricted-method and restricted-import rules).
- Keep snapshots inline. Do not commit snapshot files.
- Wait for observable conditions in tests, not with `setTimeout` or `Bun.sleep` (oxlint restricted-global and restricted-property rules).
- Write one test file per module. Do not split it into `Module.context.test.ts` files.

## Configuration

- Use edition 2024 in each package.
- Inherit the workspace's lints in each workspace member (`lints.workspace = true`).
- Take dependency versions from `[workspace.dependencies]` in each workspace member.
- Pin a version in rust-toolchain.toml, not a channel such as `stable`.
- Declare `rust-version` in the workspace.
- Set `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, and `strip = "symbols"` in the release profile.
- Add a dependency only with a stated reason. The baseline records the count without a target, so each change shows in review.
- Write source files in TypeScript, not JavaScript. Configuration files are exempt.
- Declare `sideEffects` as a boolean or a file-pattern list in each named, non-private package.
- Reference dependency versions with `catalog:` in each package.json of a pnpm workspace.
- Run TypeScript scripts directly, not through `tsx` or `ts-node`.
- Keep the dependencies and scripts of each package.json in alphabetical order.

## Commits

- Start each commit header with a conventional type, such as `feat:` or `fix:`, and a subject (commitlint `type-enum`, `type-case`, `type-empty`, and `subject-empty`).
- Start each subject in lowercase (commitlint `subject-case`).
- End each subject without a period (commitlint `subject-full-stop`).
- Keep each commit header to 100 characters or fewer (commitlint `header-max-length`).
- Put a blank line before the body and the footer, and wrap them at 100 characters (commitlint `body-leading-blank`, `body-max-line-length`, and the matching footer rules).
- State the change in the subject, not the reason. Leave out "because", "so that", and "in order to".
- Open each subject with a verb in its base form, such as "add", not "adds" or "added".
- Add or edit a test source file in each `fix:` commit.
- Write each changeset as one sentence that opens with a past-tense verb, such as "Added".
- Do not commit build output, installed packages, or `.env` files.
