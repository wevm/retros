# Deslop

Measures the architecture of a Cargo workspace with [deslop-please](https://github.com/deterministic-machines/deslop-please): deep modules and narrow interfaces. Each Rust file is an item, and a failing check carries the tool's first diagnostic for that file.

## Requirements

- **deslop-please:** Install it with `cargo install deslop-please`.
- **Bun:** Runs the script.

## Add

```sh
# Pin the pack to the newest commit of main.
uphill add wevm/uphill/packs/deslop@main
```
