/** Number formatters. */

/** Formats a number with thousands separators, such as `12,345`. */
export function grouped(value) {
  return new Intl.NumberFormat('en-US').format(value)
}

/** Formats a byte count in decimal units, such as `45.6 MB`. */
export function bytes(value) {
  const units = ['B', 'kB', 'MB', 'GB']
  const index = Math.min(units.length - 1, Math.floor(Math.log10(Math.max(1, value)) / 3))
  return `${Number((value / 1000 ** index).toFixed(1))} ${units[index]}`
}
