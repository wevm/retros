# Writes a metric, logs 25 lines to standard error, and fails.
echo 'size 1kB' >> "$RETROS_METRICS"

i=1
while [ "$i" -le 25 ]; do
  echo "build log line $i" >&2
  i=$((i + 1))
done

exit 3
