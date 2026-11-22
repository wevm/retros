/**
 * Builds the release binary and starts the server on a free port for the metric scripts. Also
 * appends to the metric file the scripts write.
 *
 * @module
 */

import * as fs from 'node:fs'

/** A running server. */
export type Server = {
  /** The base URL, such as `http://127.0.0.1:50123`. */
  url: string
  host: string
  port: number
  stop: () => void
}

/** Builds the release binary, and throws when the build fails. */
export function build(): void {
  const result = Bun.spawnSync(['cargo', 'build', '--release', '--quiet'], {
    stderr: 'inherit',
  })
  if (result.exitCode !== 0)
    throw new Error(`cargo build exited with status ${result.exitCode}`)
}

/** Builds and starts the server, and resolves once it accepts connections. */
export async function start(): Promise<Server> {
  build()

  const child = Bun.spawn(['target/release/harbor'], {
    env: { ...process.env, PORT: '0' },
    stderr: 'ignore',
  })
  const reader = child.stdout.getReader()
  let output = ''
  while (!output.includes('\n')) {
    const chunk = await reader.read()
    if (chunk.done) throw new Error('the server exited before it listened')
    output += new TextDecoder().decode(chunk.value)
  }

  const address = /listening on (\S+):(\d+)/.exec(output)!
  return {
    host: address[1]!,
    port: Number(address[2]),
    stop: () => child.kill(),
    url: `http://${address[1]}:${address[2]}`,
  }
}

/** Appends metric lines to the file at `$RETROS_METRICS`. */
export function write(lines: readonly string[]): void {
  fs.appendFileSync(process.env.RETROS_METRICS!, `${lines.join('\n')}\n`)
}
