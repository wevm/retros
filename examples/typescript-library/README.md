# TypeScript Library

tiny-format, a small library of formatters measured with the tools it already uses. Its footprint, speed, and quality categories and `metrics/` show how to measure a library.

## Fit the Pack

- **Entry points:** The scripts read `exports` in `package.json` and lint every file in `src/`. A project with built entry points measures its build output instead.
- **Call times:** `metrics/speed.ts` imports the formatters from `../src/index.js` and benchmarks each one with arguments of its own. Point the import at the project's entry point from the script's new place, and write arguments for its functions.
- **Targets:** The root entry point's target is the sum of its subpaths and 10% more. Formatters that call `Intl` get a larger call-time target of their own in `uphill.baseline.yaml`.
- **Packages:** The scripts run with Bun and call mitata, TypeScript, oxlint, and oxfmt. Swap each tool for the one the project already uses.

```sh
# Install what the scripts call, then check each category once.
bun add -d mitata typescript oxlint oxfmt @types/bun
uphill check
```
