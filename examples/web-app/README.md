# Web App

Tide, a small marketing site that Lighthouse measures in its default mobile mode. `uphill add wevm/uphill/examples/web-app` copies its four categories and `metrics/` into a project.

## Fit the Pack

- **Pages:** `metrics/lighthouse.ts` measures each `.html` file in `pages/`. Change that directory, or list the project's routes.
- **Server:** `metrics/serve.ts` serves `pages/` and bundles `src/app.ts`. Replace it with the project's own preview command, and keep the free port.
- **Targets:** The pages weigh a few kilobytes, so the targets are stricter than Lighthouse's. Start from Lighthouse's control points for a larger site.
- **Packages:** The scripts import `lighthouse` and `chrome-launcher`, and run with Bun. Lighthouse needs Chrome, or the one at `CHROME_PATH`.

```sh
# Install what the scripts import, then check each category once.
bun add -d lighthouse@13.6.0 chrome-launcher@1.2.3
uphill check
```
