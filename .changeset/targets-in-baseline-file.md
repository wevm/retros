---
'uphill': patch
---

Targets live in `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration, and each item holds its accepted value and its target side by side, such as `viem/chains: { baseline: 61kB, target: 80kB }`. A key with `*` and only a target is a pattern, `"*": { target: 5kB }` is the default, and `bundle: { target: 5kB }` targets a metric before its first update. Uphill still reads the short form `61kB target 80kB` of older baseline files. An item's own target in the file has the final say over patterns and line targets. Uphill still reads `target`, `targets`, and `baseline` keys in older configurations, and the next `uphill --update` moves them into the baseline file. `uphill add` copies a pack's default targets and patterns into the project's baseline file.
