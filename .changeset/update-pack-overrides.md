---
'uphill': patch
---

Fixed `uphill --update` in a project that overrode a pack's category. It read the configuration without its packs, so `metric: false` failed with "is not a pack's category" while a run without `--update` passed.
