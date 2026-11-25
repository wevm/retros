# Rust

The pack measures Cargo workspace members. It reads Clippy diagnostics, rustfmt results, Rust syntax trees, and manifests. It excludes nested nonmember projects and vendored packs. A failed check names the first violation and its source line when available.

If Clippy fails, its confirmed diagnostics still fail their checks. Checks without a verdict skip instead of passing. Missing Cargo metadata stops the script. `dependencies` counts distinct nonmember package IDs from `cargo build --workspace`, including build dependencies but not dev-only dependencies.

Source checks inspect syntax, not macro expansions or inferred types. Reachability uses declared Cargo targets, compiler inputs, and files rustfmt visits. Files unique to other target or feature configurations may not appear as reached.

## Requirements

- **Bun:** Runs the script and installs the pinned Rust grammar from the pack's lockfile.
- **Cargo:** The workspace's own toolchain, with Clippy and rustfmt.

## Add

The pack contributes to `correctness`, `style`, `architecture`, `documentation`, `testing`, and `configuration`.

```sh
# Pin the pack and approve the install of its parser.
uphill add wevm/uphill/packs/rust@main
```
