---
'uphill': patch
---

Replaced the GitHub Action's pull request comment with a table of score gauges, one row per category with its score on the base branch and on the pull request, and a link to `report.html`, uploaded as an unzipped artifact. The job summary kept the whole `report.md`. `uphill --compare` ran every script of the revision at least once, even with a baseline file, and `report.json` listed the revision's scores under `comparison.scores`.
