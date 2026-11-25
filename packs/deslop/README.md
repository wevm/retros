# Deslop

Measures deep modules and narrow interfaces with the four lints of [deslop-please](https://github.com/deterministic-machines/deslop-please): module cycles, tangles, leaked fields, and shallow modules. deslop-please reads a Cargo workspace, and a small scanner reads Rust without it and Zig, Go, Swift, C, and TypeScript. Each module is an item.

Its metrics go into the shared category `architecture`, which merges with the category of that name in the project and its other packs.

## Requirements

- **Bun:** Runs the script.
- **deslop-please:** Optional. With `cargo install deslop-please` and rust-analyzer, it measures Rust in place of the scanner.

## Add

```sh
# Pin the pack to the newest commit of main.
uphill add wevm/uphill/packs/deslop@main
```
