---
'uphill': patch
---

Added `uphill add <pack> [category...]`, which copied a pack's categories into `uphill.yaml` and the scripts their `run` commands named into the project. A pack was any directory with an `uphill.yaml`: a local directory, `owner/name/path` on GitHub such as `wevm/uphill/examples/web-app`, or a Git URL followed by `//path`, each with an optional `@ref`. `--as <name>` added one category under another name.
