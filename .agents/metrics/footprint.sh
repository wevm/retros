# Builds the release binary, then writes its size and the third-party crates it compiles.
set -e

cargo build --release --locked --quiet -p retros
echo "binary-size $(wc -c < target/release/retros | tr -d ' ')B" >> "$RETROS_METRICS"

# `cargo tree` repeats shared crates with a `(*)` suffix, and workspace crates show their path.
crates=$(cargo tree -p retros -e normal,build --prefix none | sed 's/ (\*)$//' | sort -u | grep -c -v '(/')
echo "dependencies $crates" >> "$RETROS_METRICS"
