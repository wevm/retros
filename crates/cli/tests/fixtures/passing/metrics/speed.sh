# Fails unless the metrics file starts empty, then writes one time per item.
if [ -s "$RETROS_METRICS" ]; then
  echo 'expected an empty metrics file' >&2
  exit 1
fi

printf 'render\t"ABI Decode (500 results)"\t1.82ms\n' >> "$RETROS_METRICS"
printf 'render encode 236us\n' >> "$RETROS_METRICS"

# Standard output is a log, so this line does not reach the metrics.
echo 'rendered 2 items'
