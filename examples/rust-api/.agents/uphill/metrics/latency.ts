/**
 * Writes how fast the server answers: the median and 95th percentile time of each endpoint over
 * 200 sequential requests, and the requests per second it serves to 16 concurrent clients. Each
 * endpoint's target comes from the work it does.
 *
 * @module
 */

import * as Server from './server.js'

/** Each endpoint and its median target in microseconds. */
const endpoints = [
  // Answer from memory without work.
  { path: '/health', target: 500 },
  { path: '/harbors/12', target: 500 },
  // Serializes all 500 harbors.
  { path: '/harbors', target: 1000 },
  // Scans every harbor name.
  { path: '/search?q=bay', target: 1000 },
  // Computes 168 hourly tide heights.
  { path: '/harbors/12/forecast', target: 2000 },
]
const requests = 200
const clients = 16

const server = await Server.start()
const lines: string[] = []

try {
  for (const endpoint of endpoints) {
    for (let index = 0; index < 20; index++) await get(endpoint.path)

    const samples: number[] = []
    for (let index = 0; index < requests; index++) {
      const start = Bun.nanoseconds()
      await get(endpoint.path)
      samples.push((Bun.nanoseconds() - start) / 1e6)
    }
    samples.sort((a, b) => a - b)

    // The tail may take three times the median's target.
    const at = (share: number) =>
      samples[Math.ceil(samples.length * share) - 1]!.toFixed(3)
    lines.push(
      `latency ${endpoint.path} ${at(0.5)}ms target ${endpoint.target}µs`,
    )
    lines.push(
      `tail-latency ${endpoint.path} ${at(0.95)}ms target ${endpoint.target * 3}µs`,
    )
  }

  let served = 0
  const until = performance.now() + 1000
  await Promise.all(
    Array.from({ length: clients }, async () => {
      while (performance.now() < until) {
        await get('/harbors/12')
        served++
      }
    }),
  )
  lines.push(`throughput ${served}`)
} finally {
  server.stop()
}

Server.write(lines)

/** Requests `path` and reads the whole response. */
async function get(path: string): Promise<void> {
  const response = await fetch(server.url + path)
  await response.arrayBuffer()
}
