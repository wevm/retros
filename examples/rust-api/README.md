# Rust API

Harbor, an HTTP API on Rust's standard library, measured with Cargo's own tools. Its footprint, latency, reliability, and testing categories and `.agents/uphill/metrics/` show how to measure an API. The rust pack adds clippy, rustfmt, panics, docs, and manifests, and its `testing` merges with the API's own.

## Fit the Pack

- **Server:** `.agents/uphill/metrics/server.ts` builds the release binary and starts `target/release/harbor` with `PORT=0`. It waits for a `listening on <host>:<port>` line. Change the binary name, and print that line from the project's server.
- **Endpoints:** `.agents/uphill/metrics/latency.ts` and `.agents/uphill/metrics/reliability.ts` list Harbor's paths, statuses, and a target per endpoint from the work it does. List the project's own endpoints.
- **Targets:** Each endpoint's 95th percentile may take three times its median target. Binary size and dependency counts fit a small server.
- **Packages:** The scripts are TypeScript run by Bun, and call `cargo`. The rust pack calls clippy and rustfmt.

```sh
# Check each category once. The first run builds the release binary.
uphill check
```
