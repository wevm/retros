# Writes one size per entry point, then the module count.
{
  echo 'export-size viem 4.2kB'
  echo 'export-size "viem/chains" 61kB'
  echo 'modules 412'
} >> "$RETROS_METRICS"
