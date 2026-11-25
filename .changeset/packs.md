---
'uphill': patch
---

Replaced copied packs with pinned, vendored ones. A top-level `packs` key listed packs such as `wevm/uphill/packs/rust@<commit>`, and Uphill ran their categories after the project's own. `uphill add` pinned a pack to a full commit and vendored it into `.agents/packs/<name>/` after a review of its files and capabilities. `uphill update` moved packs to newer commits that descend from the pinned ones, `uphill remove` removed them, and `uphill audit` compared each vendored pack with its pinned commit. Each fetch refused commits less than a day old, commits that no branch or tag contained, symbolic links, and unsafe paths, and ran no pack code. `allowed-packs` limited the sources, and `install: true` installed a pack's dependencies from its lockfile without install scripts. The repository gained the `deslop`, `docs`, `git`, `rust`, `tigerstyle`, `typescript`, and `web` packs.
