/**
 * The schedule page's script: once the page loads, builds today's departures with a slow sort on
 * the main thread, and half a second later shows a service alert, which pushes the departures
 * down.
 *
 * @module
 */

addEventListener('load', () => {
  const table = document.querySelector<HTMLTableElement>('#departures')
  const start = performance.now()
  const rows: string[] = []

  // A naive sort of every departure, which blocks the main thread for about 400 ms.
  while (performance.now() - start < 400) {
    const minutes = Math.floor(Math.random() * 1440)
    rows.push(`${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`)
    rows.sort()
  }

  for (const time of rows.slice(0, 12)) {
    const row = table?.insertRow()
    row?.insertCell().append(time)
    row?.insertCell().append('+25 min')
    row?.insertCell().append('On time')
  }

  setTimeout(() => {
    const alerts = document.querySelector('#alerts')
    if (alerts) alerts.innerHTML = '<p class="alert">Strong winds: the 18:40 to Island Pier may be cancelled. Check again before you travel.</p>'
  }, 500)
})
