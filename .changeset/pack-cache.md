---
'uphill': minor
---

Fetched remote packs into a cache outside the project, `$XDG_CACHE_HOME/uphill/packs/` or `~/.cache/uphill/packs/`, instead of vendoring them into `.agents/uphill/packs/`. The configuration's pin is the only file a project commits, and a run fetches each pinned commit that the cache lacks, with the checks of `uphill add`. `uphill audit` and the vendored pack's `local/` directory went away. The TypeScript pack reads its inputs from `.agents/uphill/typescript/config.ts`. A project with vendored packs deletes `.agents/uphill/packs/` and moves `.agents/uphill/packs/typescript/local/` to `.agents/uphill/typescript/`.
