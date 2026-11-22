# Writes a different `parse` time each round, so its median over 5 rounds is 5 ms. Throughput
# drops after the first round, and only the first round counts for a count.
round=$(( $(cat .round 2>/dev/null || echo 0) + 1 ))
echo "$round" > .round

parse=$(echo "9 1 5 3 7" | cut -d ' ' -f "$round")
throughput=2588
if [ "$round" -gt 1 ]; then
  throughput=100
fi

{
  echo "render parse ${parse}ms"
  echo "render encode 2ms"
  echo "throughput $throughput"
} >> "$RETROS_METRICS"
