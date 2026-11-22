/**
 * Writes what the server costs to ship and trust: the size of the stripped release binary, the
 * third-party crates it compiles, and the calls in its source that can panic.
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
const panics = [...new Bun.Glob('src/**/*.rs').scanSync()]
  .map(
    (file) =>
      fs
        .readFileSync(file, 'utf8')
        .match(/\.unwrap\(\)|\.expect\(|panic!\(|unreachable!\(/g)?.length ?? 0,
  )
  .reduce((total, count) => total + count, 0)

Server.write([
  `binary-size ${fs.statSync('target/release/harbor').size}B`,
  `dependencies ${metadata.packages.length - 1}`,
  `panic-sites ${panics}`,
])
