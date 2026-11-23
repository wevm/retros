# Writes a bundle size, ten export sizes whose p90 is 9 kB, a module count, and three link checks.
# Notes say why the bundle, the largest export, and the failing link are what they are.
cat >> "$RETROS_METRICS" <<'LINES'
bundle 27.5kB note "includes all of lodash"
export-size viem/actions 3kB
export-size viem/chains 10kB note "bundles every chain definition"
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
links about.html fail note "links to /team, which is gone"
links "blog/first post.html" pass
LINES
