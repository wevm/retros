# TypeScript

Measures conventions in tracked TypeScript files, including declarations and `.cts` and `.mts` modules. A source archive without Git uses the same file patterns. oxlint checks lint rules, oxc-parser reads syntax, and the TypeScript checker resolves bindings for exports, assertions, and array writes.

Its metrics go into the shared categories `correctness`, `style`, `architecture`, `documentation`, `testing`, and `configuration`. Each merges with the category of its name in the project and its other packs.

## Requirements

- **Bun:** Runs the script and installs its locked parser, linter, formatter, and TypeScript binding API.
- **Project tools:** `lint` and `formatted` use the project's linter and formatter. Each falls back to the pack's oxlint or oxfmt only if the project declares no tool. A missing or failed tool skips its metric.
- **TypeScript:** The project's `tsc` or `tsgo` supplies `type-errors` for `tsconfig.json` and its references. Files outside those programs skip. Project-wide diagnostics use the configuration file as their item.

## Add

```sh
# Pin the pack, and approve the install of its dependencies.
uphill add wevm/uphill/packs/typescript@main
```
