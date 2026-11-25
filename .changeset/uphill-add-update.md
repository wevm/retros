---
'uphill': patch
---

Added `uphill add --update [category...]`, which fetched the pack of each added category and merged its changes since the commit the category's comment recorded. A category or script that only one side changed took that side, and one that both changed was a conflict that wrote nothing unless `--force` took the pack's version. Each example gained a `README.md` that `uphill add` printed, saying what to fit.
