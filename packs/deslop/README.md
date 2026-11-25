# Deslop

Measures module cycles, type tangles, leaked fields, and shallow modules in the `architecture` category. Each item is a file, a Go package, or a C source with its header. A leak belongs to the type's declaring file. Its note names the access sites. A cycle fails each identified participant.

## Rust Coverage

At a Cargo workspace root, the pack runs `cargo check --workspace` with default features and Cargo's configured target. Only checked production targets and their compiler inputs are eligible. A failed build fails the measurement. An orphan or disabled module skips rather than increasing the score.

With deslop-please, a passing item also needs a document in rust-analyzer's SCIP index for the run. If the pack cannot identify that index, it reports native findings without assigning passes. A finding without a resolved owner belongs to `Cargo.toml`. The native tool honors `.deslop-please.allow`.

Without deslop-please, the scanner follows explicit Rust module paths and simple annotated receivers. It does not expand macros or resolve inline module cycles. Without a root Cargo manifest, it reads standalone sources, not a verified build. Each note identifies the backend.

## Scanner Coverage

- **C, Go, and Zig:** Tree-sitter identifies quoted C includes, Go import declarations, and literal Zig file imports. The scanner does not run a preprocessor, apply Go build tags or replacements, or evaluate a Zig build script.
- **Swift:** The scanner connects direct calls and named types to unique top-level declarations in the same directory. It does not resolve overloaded calls, dynamic dispatch, or Swift package targets.
- **TypeScript:** The pack's TypeScript checker resolves imports, field receivers, and called declarations, including aliases. It uses one bundler-resolution source program, not the project's build configuration.

A recognized but unresolved import skips graph-dependent checks for that language unless a confirmed violation already fails the item. An unresolved field receiver skips the candidate type's check. The scanner estimates tangles and interface width from source. A passing scanner check is not a compiler guarantee.

## Requirements

- **Bun:** Installs the pinned TypeScript checker and Tree-sitter grammars and runs the script.
- **Cargo:** Establishes the population of a Rust workspace. Rust workspace items skip if Cargo is unavailable.
- **deslop-please:** Optional. With `cargo install deslop-please` and rust-analyzer, it measures Rust in place of the scanner.

## Add

```sh
# Pin the pack and approve the install of its dependencies.
uphill add wevm/uphill/packs/deslop@main
```
