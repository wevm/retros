---
'uphill': patch
---

`uphill --update` writes baselines to `uphill.baseline.yaml` or `uphill.baseline.toml` beside the configuration, instead of each metric's `baseline` key. The configuration keeps only what people write. Uphill still reads `baseline` keys, and the next `uphill --update` moves them into the baseline file.
