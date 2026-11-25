---
'uphill': patch
---

Ran the project's own linter and formatter in the `typescript` pack. `lint` ran `bun lint`, ESLint, Biome, oxlint, or Vite Plus, and `formatted` ran `bun fmt`, Prettier, Biome, dprint, oxfmt, or Vite Plus, whichever the project used, with the project's configuration. The pack's oxlint and oxfmt ran only for a project with no linter or formatter. The `prettier` metric was removed.
