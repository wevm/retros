---
'uphill': patch
---

`uphill add --update [category...]` fetches the pack of each added category and merges its changes since the commit the category's comment records. A category or script that only one side changed takes that side, and one that both changed is a conflict that writes nothing unless `--force` takes the pack's version. Each example has a `README.md` that `uphill add` prints, saying what to fit.
