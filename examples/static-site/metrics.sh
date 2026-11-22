# Writes the size of each page and checks its local links, then writes the stylesheet size.
cd site

for page in *.html; do
  echo "page-size $page $(wc -c < "$page" | tr -d ' ')B" >> "$RETROS_METRICS"

  # Links with a `:`, such as `https:` and `mailto:`, are not local.
  result=pass
  for link in $(grep -o 'href="[^":]*"' "$page" | sed 's/^href="//; s/"$//'); do
    [ -e "$link" ] || result=fail
  done
  echo "links $page $result" >> "$RETROS_METRICS"
done

echo "css-size $(wc -c < style.css | tr -d ' ')B" >> "$RETROS_METRICS"
