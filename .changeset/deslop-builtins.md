---
'uphill': patch
---

Read runtime built-ins such as `node:fs`, and modules that only `declare module` types, as outside the project in the deslop pack, so one import of them no longer skips every TypeScript file.
