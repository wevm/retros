/**
 * Writes whether the server answers each request with the right status, rejects a malformed
 * request, keeps serving after a handler panics, and how many requests fail under concurrent load.
 *
 * @module
 */

import * as net from 'node:net'

import * as Server from './server.js'

/** Requests and the status each must answer with. */
const cases = [
  { path: '/health', status: 200 },
  { path: '/harbors/12', status: 200 },
  { path: '/harbors/12/forecast', status: 200 },
  { path: '/search?q=bay', status: 200 },
  { path: '/harbors/999', status: 404 },
  { path: '/harbors/abc', status: 400 },
  { path: '/search', status: 400 },
  { path: '/tides', status: 404 },
]

const server = await Server.start()
const lines: string[] = []

try {
  for (const entry of cases) {
    const status = await fetch(server.url + entry.path).then(
      (response) => response.status,
      () => 0,
    )
    const answered = status === 0 ? 'no answer' : `status ${status}`
    lines.push(
      status === entry.status
        ? `status ${entry.path} pass`
        : `status ${entry.path} fail note "${answered}, expected ${entry.status}"`,
    )
  }

  const reply = await raw('HELLO\r\n\r\n')
  lines.push(
    `malformed-request ${reply.startsWith('HTTP/1.1 400') ? 'pass' : 'fail'}`,
  )

  // `/search` without `q` above may have crashed a handler. The server must still answer.
  const health = await fetch(`${server.url}/health`).then(
    (response) => response.ok,
    () => false,
  )
  lines.push(`survives-panic ${health ? 'pass' : 'fail'}`)

  const paths = ['/health', '/harbors/12', '/harbors', '/search?q=bay']
  const results = await Promise.all(
    Array.from({ length: 400 }, (_, index) =>
      fetch(server.url + paths[index % paths.length]!).then(
        async (response) => {
          await response.arrayBuffer()
          return response.status < 500
        },
        () => false,
      ),
    ),
  )
  const failed = results.filter((ok) => !ok).length
  lines.push(`error-rate ${((failed / results.length) * 100).toFixed(2)}`)
} finally {
  server.stop()
}

Server.write(lines)

/** Sends raw bytes to the server and resolves with everything it answers. */
function raw(request: string): Promise<string> {
  return new Promise((resolve) => {
    let reply = ''
    const socket = net.connect(server.port, server.host, () =>
      socket.write(request),
    )
    socket.on('data', (chunk) => (reply += chunk))
    socket.on('close', () => resolve(reply))
    socket.on('error', () => resolve(reply))
  })
}
