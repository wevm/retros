# Rust

Measures Rust conventions in each source file and manifest of a Cargo workspace. clippy and rustc report what their lints cover, and the script reads the rest, such as workspace lints, the release profile, and test conventions. A failing check carries the file's first warning.

Its metrics go into the shared categories `correctness`, `style`, `architecture`, `documentation`, `testing`, and `dependencies`. Each merges with the category of its name in the project and its other packs.

## Requirements

- **Cargo:** The workspace's own toolchain, with clippy.
- **Bun:** Runs the script.

## Add

```sh
# Pin the pack to the newest commit of main.
uphill add wevm/uphill/packs/rust@main
```
