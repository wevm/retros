/** Date formatters. */

/** Formats a date as `YYYY-MM-DD`. */
export function isoDate(date) {
  return date.toISOString().slice(0, 10)
}

/** Formats the time between two dates, such as `3 days ago`. */
export function relative(date, now = new Date()) {
  const days = Math.round((now - date) / 86_400_000)
  if (days === 0) return 'today'
  return days > 0 ? `${days} days ago` : `in ${-days} days`
}
