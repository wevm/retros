# Counts its runs in runs.txt, so a test can check that both categories share each run, and
# writes zc's peak memory as the run's number in megabytes.
echo run >> runs.txt
round=$(wc -l < runs.txt | tr -d ' ')
cat >> "$RETROS_METRICS" <<LINES
zc: startup 1ms
zc: install-size 2MB
zc: turn-rss ${round}MB
ts: startup 40ms
ts: install-size 4MB
ts: turn-rss 48MB target 57MB
ts: wasm-size 250kB
LINES
