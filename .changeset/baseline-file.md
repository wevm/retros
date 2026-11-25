---
'uphill': patch
---

`uphill --update` wrote baselines to `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration, instead of each metric's `baseline` key, so the configuration kept only what people write. Uphill still read `baseline` keys, and the next `uphill --update` moved them into the baseline file.
