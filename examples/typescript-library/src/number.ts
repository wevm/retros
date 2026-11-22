/**
 * Number formatters.
 *
 * @module
 */

const units = ['B', 'kB', 'MB', 'GB']

// Creating an `Intl` formatter costs far more than formatting with one, and the first one loads
// locale data, so it is made on the first call rather than on import.
let numbers: Intl.NumberFormat | undefined

/** Formats a number with thousands separators, such as `12,345`. */
export function grouped(value: number): string {
  numbers ??= new Intl.NumberFormat('en-US')
  return numbers.format(value)
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
