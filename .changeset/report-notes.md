---
'uphill': patch
---

`uphill --note` also keeps the run's `report.json` as a Git note on the same commit, under `refs/notes/uphill-report`, so a dashboard reads each noted commit's full report from the repository. CI that pushes notes now fetches and pushes both refs with `'refs/notes/uphill*:refs/notes/uphill*'`.
