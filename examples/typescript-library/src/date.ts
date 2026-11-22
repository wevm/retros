/**
 * Date formatters.
 *
 * @module
 */

const day = 86_400_000

/** Formats a date as `YYYY-MM-DD`. */
export function isoDate(date: Date): string {
  return date.toISOString().slice(0, 10)
}

/** Formats the time between two dates, such as `3 days ago` or `in 2 days`. */
export function relative(date: Date, now = new Date()): string {
  const days = Math.round((now.getTime() - date.getTime()) / day)
  if (days === 0) return 'today'
  return days > 0 ? `${days} days ago` : `in ${-days} days`
}

export function weekday(date: Date): string {
  return date.toLocaleDateString('en-US', { timeZone: 'UTC', weekday: 'long' })
}
