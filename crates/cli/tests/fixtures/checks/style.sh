# Writes the metric lines in lines.txt, so a test edits the checks without editing the script, and
# counts its runs in .runs.
echo run >> .runs
cat lines.txt >> "$UPHILL_METRICS"
