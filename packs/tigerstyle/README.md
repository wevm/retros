# TigerStyle

Measures [TigerStyle](https://tigerstyle.dev), the TigerBeetle coding style, in each Rust, Zig, Go, Swift, C, and TypeScript file that Git tracks. clippy and oxlint measure function length where they can, and a small scanner reads assertion density, recursion, and sized integers.

## Requirements

- **Bun:** Runs the script.
- **Cargo or oxlint:** Optional. Without them, the scanner measures function length too.

## Add

```sh
# Pin the pack to the newest commit of main.
uphill add wevm/uphill/packs/tigerstyle@main
```
