---
'uphill': patch
---

Added TOML to `uphill add`. A YAML pack went into a TOML project as tables and a TOML pack into a YAML project, with the comments above each category and metric. A project without a configuration took the pack's format, and `uphill add --update` merged TOML projects too.
