# Uphill

Continuous metrics and hill climbing. Uphill runs a project's measurement scripts, scores each metric against its target, and names the fixes worth the most points.

```sh
# Run Uphill without installing it.
npx uphill

# Or add it to a project.
npm install --save-dev uphill
```

This package holds the `uphill` binaries for macOS and Linux on arm64 and x64, and runs the one for the machine. Linux runs the static musl build, which works with glibc and musl.

[github.com/wevm/uphill](https://github.com/wevm/uphill) documents configuration and usage.
