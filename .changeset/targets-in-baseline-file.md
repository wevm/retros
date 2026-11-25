---
'uphill': patch
---

Moved targets into `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration, where each item held its accepted value and its target side by side, such as `viem/chains: { baseline: 61kB, target: 80kB }`. A key with `*` and only a target became a pattern, `"*": { target: 5kB }` the default, and `bundle: { target: 5kB }` targeted a metric before its first update. An item's own target in the file took the final say over patterns and line targets. Uphill still read the short form `61kB target 80kB` of older baseline files, and the `target`, `targets`, and `baseline` keys of older configurations, which the next `uphill --update` moved into the baseline file. `uphill add` copied a pack's default targets and patterns into the project's baseline file.
