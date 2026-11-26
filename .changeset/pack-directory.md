---
'uphill': patch
---

Moved vendored packs from `.agents/packs/` to `.agents/uphill/packs/`. `uphill add` and `uphill update` wrote packs there, runs read them there, and the convention packs skipped it. A project with packs in `.agents/packs/` moved that directory with `git mv .agents/packs .agents/uphill/packs`.
