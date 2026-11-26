---
'uphill': patch
---

Kept a vendored pack's `local/` directory for the project's own files. `uphill update` kept it, `uphill audit` skipped it, `uphill remove` left it in place, and a pack that shipped `local/` failed the fetch checks. The `typescript` pack read its project inputs from `local/config.ts`.
