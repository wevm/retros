# Rust API

Harbor, an HTTP API on Rust's standard library, measured with Cargo's own tools. `uphill add wevm/uphill/examples/rust-api` copies its footprint, latency, reliability, and quality categories and `metrics/` into a project.

## Fit the Pack

- **Server:** `metrics/server.ts` builds the release binary and starts `target/release/harbor` with `PORT=0`. It waits for a `listening on <host>:<port>` line. Change the binary name, and print that line from the project's server.
- **Endpoints:** `metrics/latency.ts` and `metrics/reliability.ts` list Harbor's paths, statuses, and a target per endpoint from the work it does. List the project's own endpoints.
- **Targets:** Each endpoint's 95th percentile may take three times its median target. Binary size and dependency counts fit a small server.
- **Packages:** The scripts are TypeScript run by Bun, and call `cargo`, clippy, and rustfmt.

```sh
# Check each category once. The first run builds the release binary.
uphill check
```
