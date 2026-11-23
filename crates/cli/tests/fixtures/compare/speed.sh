# Writes encode's time from encode.txt, and parse's time from the line of parse.txt for this
# run, so each of 8 runs can write a different time.
round=$(( $(cat .round 2>/dev/null || echo 0) + 1 ))
echo "$round" > .round

{
  echo "encode $(cat encode.txt)"
  echo "parse $(sed -n "$(( (round - 1) % 8 + 1 ))p" parse.txt)ms"
} >> "$RETROS_METRICS"
