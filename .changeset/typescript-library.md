---
'uphill': patch
---

Added library categories to the `typescript` pack: `footprint`, `speed`, `coverage`, `types`, `call-types`, `type-coverage`, and `fuzz`. They measured each public export's bundle size and tree-shaking, the call and import cost of replaying the project's vitest calls, type instantiations per export and per test call, and fuzzing of each public function, with `hot-*` copies for the items `.agents/typescript.ts` listed. A metric that wrote only `skip` lines no longer had its target's unit checked, so the library metrics skipped in projects without entry points.
