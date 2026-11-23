# Writes the bundle size from bundle.txt. Fails in a checkout that setup has not prepared.
if [ ! -f .ready ]; then
  echo "run setup first" >&2
  exit 1
fi
echo "bundle $(cat bundle.txt)" >> "$UPHILL_METRICS"
