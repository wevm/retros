# TigerStyle

Measures [TigerStyle](https://tigerstyle.dev) in Git-tracked Rust, Zig, Go, Swift, C, and TypeScript source. The pack uses each language's syntax tree to find function spans, calls, and assertions. Each source file is a check item. Generated directories, vendored packs, TypeScript declarations, and untracked files are excluded.

The `correctness`, `style`, and `configuration` categories merge with the project's categories of the same name.

## Requirements

- **Bun and Git:** Bun installs the pinned Tree-sitter runtime and WebAssembly grammars. Git identifies the files to measure.
- **Clippy and oxlint:** Optional. Their length diagnostics supplement the source spans. A failed command, ignored path, or unrelated diagnostic does not certify a file's length.

## Function Checks

The 70-line limit includes the signature, comments, and blank lines. `function-length` checks the whole file. `function-lines` records each function's exact span under an item such as `src/queue.rs::push`. A change from 83 to 74 lines appears even though the file still fails.

`assertion-density` targets two assertion calls per named production function on average. It excludes calls at file scope, test files, Rust test items, and Zig test blocks. It has no value without a denominator. `compound-assertions` inspects the condition, not the assertion message or words inside a string.

`recursion` checks direct self-calls, including explicit generic arguments and method receivers. It does not resolve aliases, overloads, macro expansions, or cycles between functions. A syntax error fails `source-coverage`. Checks that require the incomplete tree skip rather than pass.

## Manifest Checks

`zero-dependencies` reads Cargo dependencies, including target tables, direct Go requirements, npm runtime, optional and peer dependencies, Zig package entries, and literal Swift package dependencies. It excludes Cargo development and build dependencies and indirect Go requirements. It does not execute Swift manifests or infer libraries from C source.

## Add

```sh
# Pin the pack to the newest commit of main.
uphill add wevm/uphill/packs/tigerstyle@main
```

For an existing or local pack entry, set `install: true` to approve the lockfile install.

```yaml
# Install the local pack's grammars before the first run.
packs:
  - { pack: ./packs/tigerstyle, install: true }
```
