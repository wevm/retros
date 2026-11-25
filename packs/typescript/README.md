# TypeScript

Measures TypeScript conventions in each `.ts` and `.tsx` file that Git tracks. oxlint reports each convention that one of its rules covers, and the script reads the rest from oxc-parser's syntax tree. Type errors and formatting come from the project's own TypeScript and oxfmt.

## Requirements

- **Bun:** Runs the script and installs oxlint and oxc-parser from the pack's lockfile.
- **TypeScript:** The project's `tsc` or `tsgo`, for `type-errors`. It skips without one.

## Add

```sh
# Pin the pack, and approve the install of its dependencies.
uphill add wevm/uphill/packs/typescript@main
```
