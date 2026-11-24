---
'uphill': patch
---

`uphill add <pack> [category...]` copies a pack's categories into `uphill.yaml` and the scripts their `run` commands name into the project. A pack is any directory with an `uphill.yaml`: a local directory, `owner/name/path` on GitHub such as `wevm/uphill/examples/web-app`, or a Git URL followed by `//path`, each with an optional `@ref`. `--as <name>` adds one category under another name.
