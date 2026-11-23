# Retros

Continuous metrics and hill climbing. Retros runs a project's measurement scripts, scores each metric against its target, and names the fixes worth the most points.

```sh
# Run Retros without installing it.
npx retros

# Or add it to a project.
npm install --save-dev retros
```

This package runs the `retros` binary from the platform package that matches the machine, such as `@retros/linux-x64-gnu`. Retros has binaries for macOS and Linux on arm64 and x64.

See [github.com/wevm/retros](https://github.com/wevm/retros) for configuration and usage.
