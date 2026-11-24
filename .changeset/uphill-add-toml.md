---
'uphill': patch
---

`uphill add` copies packs into and out of `uphill.toml`. A YAML pack goes into a TOML project as tables and a TOML pack into a YAML project, with the comments above each category and metric. A project without a configuration takes the pack's format, and `uphill add --update` merges TOML projects too.
