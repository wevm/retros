/**
 * Number formatters.
 *
 * @module
 */

const units = ['B', 'kB', 'MB', 'GB']

/** Formats a number with thousands separators, such as `12,345`. */
export function grouped(value: number): string {
  // Creating an `Intl.NumberFormat` costs far more than formatting with one. JavaScriptCore, Bun's
  // engine, caches the formatter behind `toLocaleString`, and import creates none.
  return value.toLocaleString('en-US')
}

/** Formats a byte count in decimal units, such as `45.6 MB`. */
export function bytes(value: number): string {
  const index = Math.min(
    units.length - 1,
    Math.floor(Math.log10(Math.max(1, value)) / 3),
  )
  return `${Number((value / 1000 ** index).toFixed(1))} ${units[index]}`
}

/** Formats a share from 0 to 1 as a percentage, such as `42%`. */
export function percent(value: number): string {
  return `${Math.round(value * 100)}%`
}
