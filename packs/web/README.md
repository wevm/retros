# Web

Measures a website with the tools that cover each convention. Lighthouse audits each page of the built site in `dist/`, oxlint's `jsx-a11y` rules read each component, and stylelint reads each stylesheet. A metric skips when the project has none of the files it reads.

The focus check pairs authored selectors within a stylesheet. The motion check looks for a matching declaration under `prefers-reduced-motion`. Each selector is a separate item. Ignored stylesheets skip. A parser error fails the run instead of passing rules that did not run.

Its metrics go into the shared categories `performance`, `accessibility`, and `best-practices`. Each merges with the category of its name in the project and its other packs.

## Requirements

- **Bun:** Runs the script and installs Lighthouse, oxlint, and stylelint from the pack's lockfile.
- **Chrome:** Lighthouse reads `CHROME_PATH`, or finds an installed Chrome.
- **A build:** The site in `dist/`. The script runs the `build` script of the project's `package.json` first, when it has one.

## Add

```sh
# Pin the pack, and approve the install of its dependencies.
uphill add wevm/uphill/packs/web@main
```
