# Writes a bundle size, ten export sizes whose p90 is 9 kB, a module count, and three link checks.
cat >> "$RETROS_METRICS" <<'LINES'
bundle 27.5kB
export-size viem/actions 3kB
export-size viem/chains 10kB
export-size viem 1kB
export-size viem/utils 8kB
export-size viem/siwe 5kB
export-size viem/accounts 2kB
export-size viem/op-stack 9kB
export-size viem/ens 4kB
export-size viem/zksync 7kB
export-size viem/window 6kB
modules 412
links index.html pass
links about.html fail
links "blog/first post.html" pass
LINES
