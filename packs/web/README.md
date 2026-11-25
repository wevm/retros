# Web

Measures a website with the tools that cover each convention. Lighthouse audits each page of the built site in `dist/`, oxlint's `jsx-a11y` rules read each component, and stylelint reads each stylesheet. A metric skips when the project has none of the files it reads.

## Requirements

- **Bun:** Runs the script and installs Lighthouse, oxlint, and stylelint from the pack's lockfile.
- **Chrome:** Lighthouse reads `CHROME_PATH`, or finds an installed Chrome.
- **A build:** The site in `dist/`. Override `run` to build first.

## Add

```sh
# Pin the pack, and approve the install of its dependencies.
uphill add wevm/uphill/packs/web@main
```

```yaml
# Build the site before the pack's script runs.
web:
  run: pnpm build && bun .agents/packs/web/metrics/web.ts
```
