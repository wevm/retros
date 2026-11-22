/**
 * Writes what the server costs to ship: the size of the stripped release binary, and the
 * third-party crates `cargo metadata` resolves for it.
 *
 * @module
 */

import * as fs from 'node:fs'

import * as Server from './server.js'

Server.build()

const metadata = JSON.parse(
  Bun.spawnSync([
    'cargo',
    'metadata',
    '--format-version',
    '1',
  ]).stdout.toString(),
)

Server.write([
  `binary-size ${fs.statSync('target/release/harbor').size}B`,
  `dependencies ${metadata.packages.length - 1}`,
])
