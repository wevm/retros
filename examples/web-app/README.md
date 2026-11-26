# Web App

Tide, a small marketing site that Lighthouse measures in its default mobile mode. Its performance and SEO categories, the web pack's accessibility and best practices, and `.agents/uphill/metrics/` show how to measure a site. The pack's `performance` merges with the site's own.

## Fit the Pack

- **Pages:** `.agents/uphill/metrics/lighthouse.ts` measures each `.html` file in `pages/`. Change that directory, or list the project's routes.
- **Server:** `.agents/uphill/metrics/serve.ts` serves `pages/` and bundles `src/app.ts`. Replace it with the project's own preview command, and keep the free port.
- **Build:** The `build` script in `package.json` writes the site to `dist/`, which the web pack runs before it reads the site. Point it at the project's own build.
- **Targets:** The pages weigh a few kilobytes, so the targets are stricter than Lighthouse's. Start from Lighthouse's control points for a larger site.
- **Packages:** The scripts import `lighthouse` and `chrome-launcher`, and run with Bun. Lighthouse needs Chrome, or the one at `CHROME_PATH`.

```sh
# Install what the scripts import, then check each category once.
bun add -d lighthouse@13.6.0 chrome-launcher@1.2.3
uphill check
```
