/**
 * Node process that `fuzz.ts` starts. The library runs under Node here, as its users run it, from
 * the build library.ts names.
 *
 * - `node fuzz-worker.ts plan <corpus.json> <out.json>` walks every entry point `entries()` in
 *   library.ts lists, as speed-worker.ts does, so each distinct public function (namespace
 *   members) has the same name as in the other categories, and lists the harnesses each one gets,
 *   or why it gets none, and the further names that reach a function, such as `lib:Math.sum` for
 *   `lib:Array.sum`.
 * - `node fuzz-worker.ts fuzz <job.json> <out.jsonl>` fuzzes the job's functions in turn and
 *   appends one JSON line per function. With `trace` set, it writes each input to the trace file
 *   before calling, so the orchestrator can name the input that hung or exhausted memory.
 * - `node fuzz-worker.ts corpus <recording.json> <out.json>` snapshots the speed recording.
 *
 * A function's harnesses are the structured properties in the project's properties module
 * (`fuzz.properties`) that cover it, such as round trips, re-encoding of recorded values,
 * differentials against reference implementations, and consistency, boundary mutations of the
 * arguments the project's tests passed it (the corpus), and boundary mutations of hand-written
 * seeds. Properties, seeds and the corpus may name a function by any path the entries reach it by,
 * such as `lib:Math.add` or `lib/Math:add`. V8 precise coverage, read through node:inspector after
 * each function, gives the share of the function's own lines that ran.
 *
 * @module
 */

import * as childProcess from 'node:child_process'
import * as fs from 'node:fs'
import * as inspector from 'node:inspector/promises'
import * as nodeModule from 'node:module'
import * as path from 'node:path'
import * as url from 'node:url'

import * as acorn from 'acorn'
import fc from 'fast-check'

import {
  type Base,
  type Crash,
  create,
  Failure,
  type Fuzzer,
  hash32,
  homogeneous,
  isSchema,
  keepsType,
  type Mutant,
  type Properties,
  type Property,
  prng,
  type Shapes,
  short,
  site,
} from './fuzz-lib.ts'
import * as library from './library.ts'
import { decode, keys } from './speed-codec.ts'

const [mode, ...rest] = process.argv.slice(2)

const config = library.config.fuzz ?? {}

/** The project's properties module, or none. */
const project: Properties = config.properties
  ? await import(
      url.pathToFileURL(path.resolve(library.root, config.properties)).href
    )
  : {}
const budgets = project.budgets ?? []
const mutations = project.mutations ?? 500
const properties = project.properties ?? (() => [])
const seeds = project.seeds ?? (() => ({}))

// An arbitrary given only a `maxLength` stays at size 'small', about 10 elements, so long-input
// branches (UTF-8 past 64 bytes, CBOR lengths of 24 and up) would never run. Size to the maximum.
fc.configureGlobal({
  ...fc.readConfigureGlobal(),
  defaultSizeToMaxWhenMaxSpecified: true,
})

/** A promise that has not settled after this long is a hang. */
const asyncTimeoutMs = 4_000
/** A synchronous call that takes longer is a hang. */
const slowCallMs = 3_000
/**
 * Longer limits for work that is slow by design, such as key derivation whose cost is its
 * iteration count, from `fuzz.slow`.
 */
const limitOf = (name: string) =>
  config.slow?.find((entry) => entry[0].test(name))?.[1]
/** Distinct failures kept per function. */
const failuresPerFunction = 3
/** Attempts the minimizer makes per failure. */
const minimizeAttempts = 150

/** Built-in errors a function documents, from `fuzz.documentedErrors`. */
const documentedErrors = new Map(Object.entries(config.documentedErrors ?? {}))

type Item = {
  name: string
  entry: string
  fn: (...args: readonly unknown[]) => unknown
  self: unknown
  package?: string | undefined
  params: string
}

type Location = { url: string; line: number; column: number }

/** What `plan` writes per function. */
export type Planned = {
  /** The function's item name, such as `lib:Math.add`. */
  name: string
  /** The function's package. The plan skips dependency functions, so it stays `undefined`. */
  package?: string | undefined
  /** The IDs of the structured properties that cover the function. */
  properties: readonly string[]
  /** The number of recorded and hand-written argument sets to mutate. */
  seeds: number
  /** Why the function has no harness. */
  reason?: string | undefined
  /** The function's source location, which scopes its line coverage. */
  location?: Location | undefined
}

/** What `fuzz` writes per function. */
export type Result = {
  /** The function's item name, such as `lib:Math.add`. */
  name: string
  /** Each harness that ran, with its oracle and its number of runs. */
  harnesses: { id: string; oracle: string; runs: number }[]
  /** The failures the harnesses found, up to a cap and one per distinct cause. */
  failures: FailureRecord[]
  /** The line coverage of the function's own source, when its location is known. */
  coverage?: { percent: number; lines: number; covered: number } | undefined
  /** The wall time of the function's harnesses, in milliseconds. */
  ms: number
  /** The number of inputs the harnesses passed to the function. */
  calls: number
}

/** One failure a harness found. */
export type FailureRecord = {
  /** The oracle that failed, such as `crash` or `round trip`. */
  oracle: string
  /** The ID of the harness that found the failure. */
  harness: string
  /** The kind of failure, such as `stack overflow` or an error's class name. */
  kind: string
  /** The failing input, formatted for a metric note. */
  input: string
  /** The error message, shortened. */
  message: string
  /** The `file:line` of the throw, when the stack names one. */
  site?: string | undefined
}

/**
 * The library's entry points in library.ts's order, each distinct function named after the first
 * entry that reaches it (`lib:Math.add`, not `lib/Math:add`), and `nameOf`, which maps any path
 * the entries reach a function by to that name.
 */
async function loadEntries() {
  const modules: Record<string, Record<string, unknown>> = {}
  const items: Item[] = []
  const byFunction = new Map<unknown, Item>()
  const walked = new Set<unknown>()
  /** Where the walk reaches a value: the entry, the path from it, and the object that holds it. */
  type Walk = { entry: string; name: string; self: unknown }
  const walk = (value: unknown, at: Walk) => {
    const { entry, name, self } = at
    if (typeof value === 'function') {
      if (isClass(value)) {
        walked.add(value)
        return
      }
      if (walked.has(value)) return
      if (byFunction.has(value)) return
      const item: Item = {
        name: `${entry}:${name}`,
        entry,
        fn: value as Item['fn'],
        self,
        params: paramsOf(value),
      }
      items.push(item)
      byFunction.set(value, item)
      return
    }
    if (isNamespace(value)) {
      if (walked.has(value)) return
      walked.add(value)
      for (const [key, member] of Object.entries(value))
        walk(member, {
          entry,
          name: name ? `${name}.${key}` : key,
          self: value,
        })
      return
    }
    if (value && typeof value === 'object') walked.add(value)
  }
  for (const entry of library.entries()) {
    const module = await import(url.pathToFileURL(entry.file).href)
    modules[entry.name] = module
    if (walked.has(module)) continue
    walked.add(module)
    for (const [key, value] of Object.entries(module))
      walk(value, { entry: entry.name, name: key, self: module })
  }
  const names = new Map<string, string | undefined>()
  const nameOf = (key: string) => {
    if (!names.has(key)) {
      const colon = key.indexOf(':')
      let value: unknown = modules[key.slice(0, colon)]
      for (const part of key.slice(colon + 1).split('.'))
        value =
          value && (typeof value === 'object' || typeof value === 'function')
            ? (value as Record<string, unknown>)[part]
            : undefined
      names.set(key, byFunction.get(value)?.name)
    }
    return names.get(key)
  }
  return { modules, items, nameOf }
}

/**
 * The library handle properties use: the library's entries, fast-check, and the recorded
 * arguments, to which the properties module's `library` adds what its properties need, such as
 * reference implementations.
 */
async function loadLib(fuzzer: Fuzzer, options: loadLib.Options) {
  const { modules, nameOf, recorded } = options
  const base: Base = {
    BaseError: (() => {
      if (!config.baseError) return Error

      // `nameOf` names functions only, and the walk skips classes, so resolve the path here.
      const colon = config.baseError.indexOf(':')
      let value: unknown = modules[config.baseError.slice(0, colon)]
      for (const part of config.baseError.slice(colon + 1).split('.'))
        value = (value as Record<string, unknown> | undefined)?.[part]
      if (typeof value !== 'function')
        throw new Error(`fuzz.baseError: ${config.baseError} is not a class.`)
      return value as Base['BaseError']
    })(),
    fc,
    modules,
    // oxlint-disable-next-line eslint/max-params -- Properties modules call `mutate` positionally, so its signature is a public contract.
    mutate(seed, pool, random, count) {
      const mutant = fuzzer.mutate(seed, {
        count,
        random: prng(random),
        shapes: fuzzer.shapesOf(pool),
      })
      return mutant.typed ? mutant.args : undefined
    },
    recorded: (name) => recorded.get(nameOf(name) ?? name) ?? [],
    root: library.root,
  }
  return project.library ? project.library(base) : base
}

declare namespace loadLib {
  /** The library's entry modules, the recorded argument sets, and the item names. */
  type Options = {
    modules: Record<string, Record<string, unknown>>
    nameOf: (key: string) => string | undefined
    recorded: ReadonlyMap<string, readonly (readonly unknown[])[]>
  }
}

/** The value a recorded key (`src/<file>.ts#<export path>`) names in the build. */
async function resolveKey(key: string): Promise<unknown> {
  const [file, exportPath] = key.split('#') as [string, string]
  let value: unknown = await import(
    url.pathToFileURL(library.built(file)).href
  ).catch(() => null)
  for (const part of exportPath.split('.'))
    value =
      value && (typeof value === 'object' || typeof value === 'function')
        ? (value as Record<string, unknown>)[part]
        : undefined
  return value
}

type Corpus = {
  functions: Record<string, { seeds: readonly string[] }>
}

/**
 * Each public function's recorded argument sets (speed-codec's text), under its name. A corpus
 * recorded on another walk names functions otherwise, such as `lib/Math:add`, and may list a
 * function under two names; its sets are merged.
 */
function corpusSeeds(
  corpus: Corpus,
  nameOf: (key: string) => string | undefined,
) {
  const out = new Map<string, readonly string[]>()
  for (const [key, { seeds }] of Object.entries(corpus.functions)) {
    const item = nameOf(key)
    if (!item) continue
    const texts = [...(out.get(item) ?? [])]
    for (const text of seeds) if (!texts.includes(text)) texts.push(text)
    out.set(item, texts)
  }
  return out
}

/** Rebuilds recorded argument sets, skipping any that cannot be rebuilt. */
async function seedsFor(fuzzer: Fuzzer, texts: readonly string[]) {
  const exports = new Map<string, unknown>()
  for (const text of texts)
    for (const key of keys(text))
      if (!exports.has(key)) exports.set(key, await resolveKey(key))
  const rebuild = {
    ref: (key: string) => exports.get(key),
    call: (key: string, args: readonly unknown[]) => {
      const fn = exports.get(key)
      if (typeof fn !== 'function') throw new Error(`${key} is not a function`)
      return fn(...args)
    },
    construct: (key: string, args: readonly unknown[]) => {
      const fn = exports.get(key)
      if (typeof fn !== 'function') throw new Error(`${key} is not a class`)
      return Reflect.construct(fn, args)
    },
  }
  const seeds: (readonly unknown[])[] = []
  for (const text of texts) {
    try {
      const origins = new Map<object, unknown>()
      const args = await decode(text, { origins, rebuild })
      fuzzer.markRebuilt(origins)
      seeds.push(args)
    } catch {}
  }
  return seeds
}

/** Hand-written argument sets by function name, from the properties module's `seeds`. */
function writtenSeeds(
  written: Record<string, readonly (readonly unknown[])[]>,
  nameOf: (key: string) => string | undefined,
) {
  const out = new Map<string, readonly (readonly unknown[])[]>()
  for (const [key, sets] of Object.entries(written)) {
    const item = nameOf(key) ?? key
    out.set(item, [...(out.get(item) ?? []), ...sets])
  }
  return out
}

/** The public functions a property covers, by name. */
function coveredBy(
  prop: Property,
  nameOf: (key: string) => string | undefined,
) {
  return [
    ...new Set(
      prop.covers.flatMap((cover) => {
        const item = nameOf(cover)
        return item ? [item] : []
      }),
    ),
  ]
}

/** Recorded argument sets kept per function, and the longest kept. */
const seedsPerFunction = 10
const longestSeed = 40_000

/**
 * `node fuzz-worker.ts corpus <recording.json> <out.json>` snapshots the speed category's
 * recording (the arguments the project's tests passed each export, see speed-codec.ts) into the
 * fuzz corpus, keyed by public function. The file stays within `fuzz.maxLength` characters, and
 * is formatted with the `fuzz.format` command.
 */
async function buildCorpus() {
  const [recordingFile, output] = rest as [string, string]
  type Sample = { args: string; direct: boolean; n: number; where: string }
  const recording = JSON.parse(fs.readFileSync(recordingFile, 'utf8')) as {
    hash: string
    createdAt: string
    keys: Record<string, { ok: readonly Sample[] }>
  }
  const { items } = await loadEntries()
  const byFunction = new Map(items.map((item) => [item.fn as unknown, item]))
  const samples = new Map<string, readonly Sample[]>()
  for (const [key, value] of Object.entries(recording.keys)) {
    const item = byFunction.get(await resolveKey(key))
    if (!item) continue
    samples.set(item.name, [...(samples.get(item.name) ?? []), ...value.ok])
  }
  const functions: Record<string, { seeds: string[]; where: string[] }> = {}
  for (const name of [...samples.keys()].sort()) {
    const unique = new Map<string, Sample>()
    // A recorded client (`$client`) cannot be rebuilt.
    for (const sample of samples.get(name)!)
      if (
        sample.args.length <= longestSeed &&
        !sample.args.includes('"$client"') &&
        !unique.has(sample.args)
      )
        unique.set(sample.args, sample)
    // Calls from tests first, then calls other library functions made; in each, spread over sizes.
    const pick = (group: Sample[], count: number) => {
      const sorted = group.sort(
        (a, b) => a.args.length - b.args.length || (a.args < b.args ? -1 : 1),
      )
      if (sorted.length <= count) return sorted
      if (count <= 1) return sorted.slice(sorted.length >> 1).slice(0, count)
      return Array.from(
        { length: count },
        (_, i) => sorted[Math.round((i * (sorted.length - 1)) / (count - 1))]!,
      )
    }
    const all = [...unique.values()]
    const direct = pick(
      all.filter((sample) => sample.direct),
      seedsPerFunction - 2,
    )
    const indirect = pick(
      all.filter((sample) => !sample.direct),
      seedsPerFunction - direct.length,
    )
    const chosen = [...new Set([...direct, ...indirect])]
    if (chosen.length)
      functions[name] = {
        seeds: chosen.map((sample) => sample.args),
        where: chosen.map((sample) => sample.where),
      }
  }
  // Over the limit, drop the longest seed of the function with the most until it fits.
  const corpusSize = () =>
    Object.values(functions).reduce(
      (total, entry) =>
        total + entry.seeds.join('').length * 1.2 + entry.where.join('').length,
      0,
    )
  while (corpusSize() > (config.maxLength ?? Number.POSITIVE_INFINITY)) {
    const [name, entry] = Object.entries(functions).sort(
      (left, right) =>
        right[1].seeds.length - left[1].seeds.length ||
        (left[0] < right[0] ? -1 : 1),
    )[0]!
    if (entry.seeds.length <= 1) break
    // The first of the longest seeds.
    const lengths = entry.seeds.map((seed) => seed.length)
    const longest = lengths.indexOf(Math.max(...lengths))
    entry.seeds.splice(longest, 1)
    entry.where.splice(longest, 1)
    functions[name] = entry
  }
  fs.writeFileSync(
    output,
    `${JSON.stringify(
      {
        source: `speed recording ${recording.hash} (${recording.createdAt})`,
        functions,
      },
      null,
      2,
    )}\n`,
  )
  const [command, ...args] = config.format ?? []
  if (command)
    try {
      childProcess.execFileSync(
        path.resolve(library.root, command),
        [...args, output],
        {
          cwd: library.root,
        },
      )
    } catch {}
}

async function plan() {
  const [corpusFile, output] = rest as [string, string]
  blockNetwork()
  const corpus = JSON.parse(fs.readFileSync(corpusFile, 'utf8')) as Corpus
  const fuzzer = create()
  const { modules, items, nameOf } = await loadEntries()
  const texts = corpusSeeds(corpus, nameOf)
  const recorded = new Map<string, readonly (readonly unknown[])[]>()
  for (const item of items)
    recorded.set(item.name, await seedsFor(fuzzer, texts.get(item.name) ?? []))
  const lib = await loadLib(fuzzer, { modules, nameOf, recorded })
  const props = properties(lib)
  const unknownCovers = props.flatMap((prop) =>
    prop.covers
      .filter((name) => !nameOf(name))
      .map((name) => `${prop.id}: ${name}`),
  )
  const byName = new Map<string, readonly Property[]>()
  for (const prop of props)
    for (const name of coveredBy(prop, nameOf))
      byName.set(name, [...(byName.get(name) ?? []), prop])

  const locations = await functionLocations(items)
  const synthetic = writtenSeeds(seeds(lib), nameOf)
  const planned: Planned[] = []
  for (const item of items) {
    const seedCount =
      recorded.get(item.name)!.length + (synthetic.get(item.name)?.length ?? 0)
    const own = byName.get(item.name) ?? []
    const location = locations.get(item.fn)
    item.package = location
      ? location.url.match(/.*\/node_modules\/((?:@[^/]+\/)?[^/]+)\//)?.[1]
      : 'built-in'
    // A dependency's own functions, which an entry re-exports, are the dependency's to fuzz, as
    // speed-worker.ts leaves them out of the bench.
    if (item.package) continue
    planned.push({
      name: item.name,
      package: item.package,
      properties: own.map((prop) => prop.id),
      seeds: seedCount,
      reason:
        own.length || seedCount
          ? undefined
          : 'no recorded call to mutate and no structured harness',
      location,
    })
  }
  fs.writeFileSync(
    output,
    JSON.stringify({
      items: planned,
      properties: props.map((prop) => ({
        id: prop.id,
        oracle: prop.oracle,
        covers: prop.covers,
      })),
      unknownCovers,
    }),
  )
}

/** Each function's source location, from its `[[FunctionLocation]]`. */
async function functionLocations(items: readonly Item[]) {
  const session = new inspector.Session()
  session.connect()
  const scripts = new Map<string, string>()
  session.on('Debugger.scriptParsed', (message) =>
    scripts.set(message.params.scriptId, message.params.url),
  )
  await session.post('Debugger.enable')
  const locations = new Map<unknown, Location>()
  const probe = globalThis as { __fuzzProbe?: unknown }
  for (const item of items) {
    probe.__fuzzProbe = item.fn
    const { result } = await session.post('Runtime.evaluate', {
      expression: 'globalThis.__fuzzProbe',
    })
    const { internalProperties } = await session.post('Runtime.getProperties', {
      objectId: result.objectId!,
    })
    const location = internalProperties?.find(
      (property) => property.name === '[[FunctionLocation]]',
    )?.value?.value as
      | { scriptId: string; lineNumber: number; columnNumber: number }
      | undefined
    const url = location ? scripts.get(location.scriptId) : undefined
    if (location && url)
      locations.set(item.fn, {
        url,
        line: location.lineNumber,
        column: location.columnNumber,
      })
  }
  await session.post('Debugger.disable')
  session.disconnect()
  return locations
}

type Job = {
  items: readonly string[]
  corpus: string
  locations: Record<string, Location>
  trace?: string | undefined
}

async function fuzz() {
  const [jobFile, output] = rest as [string, string]
  const job = JSON.parse(fs.readFileSync(jobFile, 'utf8')) as Job
  // A fuzzed call can leave a timer or promise that fails later; that is not the worker's error.
  process.on('unhandledRejection', () => {})
  blockNetwork()
  // The library's caches, emptied before each function so runs do not depend on order.
  const clearCaches = (await library.config.caches?.(() => {})) ?? (() => {})
  stubWindow()
  const session = new inspector.Session()
  session.connect()
  await session.post('Profiler.enable')
  await session.post('Profiler.startPreciseCoverage', {
    callCount: true,
    detailed: true,
  })

  const corpus = JSON.parse(fs.readFileSync(job.corpus, 'utf8')) as Corpus
  const fuzzer = create()
  // The sources that coverage reads, parsed once per worker.
  const sources = new Map<string, Source>()
  const { modules, items, nameOf } = await loadEntries()
  // Recorded arguments are rebuilt up front: properties re-encode them too.
  const texts = corpusSeeds(corpus, nameOf)
  const recorded = new Map<string, readonly (readonly unknown[])[]>()
  for (const name of job.items)
    recorded.set(name, await seedsFor(fuzzer, texts.get(name) ?? []))
  const lib = await loadLib(fuzzer, { modules, nameOf, recorded })
  const props = properties(lib).map((prop) => ({
    prop,
    covers: coveredBy(prop, nameOf),
  }))
  const synthetic = writtenSeeds(seeds(lib), nameOf)
  const byName = new Map(items.map((item) => [item.name, item]))
  const traceFd = job.trace ? fs.openSync(job.trace, 'w') : undefined
  await session.post('Profiler.takePreciseCoverage')

  for (const name of job.items) {
    const item = byName.get(name)
    if (!item) throw new Error(`${name} is not a public function`)
    const started = performance.now()
    fs.appendFileSync(output, `${JSON.stringify({ start: name })}\n`)
    clearCaches()
    // Code optimized in an earlier window stops counting once the counters reset, so a small
    // function an earlier function made hot would read as never run. Restarting precise coverage
    // deoptimizes everything, and every window starts counting from scratch.
    await session.post('Profiler.stopPreciseCoverage')
    await session.post('Profiler.startPreciseCoverage', {
      callCount: true,
      detailed: true,
    })
    const result: Result = {
      name,
      harnesses: [],
      failures: [],
      ms: 0,
      calls: 0,
    }
    const add = (failure: FailureRecord) => {
      const signature = `${failure.oracle}\0${failure.kind}\0${failure.site ?? failure.message}`
      if (
        result.failures.length < failuresPerFunction &&
        !result.failures.some(
          (other) =>
            `${other.oracle}\0${other.kind}\0${other.site ?? other.message}` ===
            signature,
        )
      )
        result.failures.push(failure)
    }

    for (const { prop, covers } of props) {
      if (!covers.includes(name)) continue
      const runs = await runProperty(prop, {
        add,
        fuzzer,
        name,
        nameOf,
        traceFd,
      })
      result.harnesses.push({ id: prop.id, oracle: prop.oracle, runs })
      result.calls += runs
    }
    const own = recorded.get(name) ?? []
    if (own.length) {
      const calls = await runCorpus(item, { add, fuzzer, seeds: own, traceFd })
      result.harnesses.push({
        id: `corpus (${own.length} recorded argument sets)`,
        oracle: 'crash',
        runs: calls,
      })
      result.calls += calls
    }
    // Hand-written seeds only count when they run as written: a wrong seed is the fuzzer's bug.
    const written: (readonly unknown[])[] = []
    for (const args of synthetic.get(name) ?? []) {
      fuzzer.markOpaque(args)
      try {
        let value: unknown = Reflect.apply(
          item.fn,
          item.self,
          fuzzer.clone(args),
        )
        if (isThenable(value)) value = await withTimeout(value, asyncTimeoutMs)
        written.push(args)
      } catch {}
    }
    if (written.length) {
      const calls = await runCorpus(item, {
        add,
        fuzzer,
        seeds: written,
        traceFd,
      })
      result.harnesses.push({
        id: `seeds (${written.length} hand-written argument sets)`,
        oracle: 'crash',
        runs: calls,
      })
      result.calls += calls
    }

    const { result: coverage } = await session.post(
      'Profiler.takePreciseCoverage',
    )
    const location = job.locations[name]
    if (location)
      result.coverage = coverageOf(
        coverage as unknown as readonly ScriptCoverage[],
        { location, sources },
      )
    result.ms = performance.now() - started
    fs.appendFileSync(output, `${JSON.stringify(result)}\n`)
  }
  const usage = process.resourceUsage()
  fs.appendFileSync(
    output,
    `${JSON.stringify({ done: true, maxRss: usage.maxRSS * 1024 })}\n`,
  )
}

/** Runs one property with its own fixed seed and run count, and records its failure. */
async function runProperty(prop: Property, options: runProperty.Options) {
  const { add, fuzzer, name, nameOf, traceFd } = options
  const check = (input: unknown) => {
    if (traceFd !== undefined)
      fs.writeSync(
        traceFd,
        `${JSON.stringify({ harness: prop.id, input: fuzzer.format(input, 2000) })}\n`,
      )
    return prop.check(input)
  }
  const property = prop.async
    ? fc.asyncProperty(prop.arb, async (input) => {
        await withTimeout(Promise.resolve(check(input)), asyncTimeoutMs * 5)
      })
    : fc.property(prop.arb, (input) => {
        check(input)
      })
  const details = await fc.check(property as fc.IAsyncProperty<unknown>, {
    seed: hash32(prop.id) | 0,
    numRuns: prop.runs,
    endOnFailure: false,
  })
  if (details.failed) {
    const error = details.errorInstance as unknown
    const blame = error instanceof Failure ? error.blame : undefined
    if (!blame || blame.some((entry) => nameOf(entry) === name)) {
      const failure = error instanceof Failure ? error : undefined
      add({
        oracle: prop.oracle,
        harness: prop.id,
        kind: failure?.kind ?? (error as Error)?.name ?? 'failed',
        input: fuzzer.format(details.counterexample?.[0], 600),
        message: short(
          failure?.message ?? String((error as Error)?.message ?? error),
          300,
        ),
        site: failure?.site ?? site(error),
      })
    }
  }
  return details.numRuns
}

declare namespace runProperty {
  /** The function under test, where failures go, and the trace file. */
  type Options = {
    add: (failure: FailureRecord) => void
    fuzzer: Fuzzer
    name: string
    nameOf: (key: string) => string | undefined
    traceFd: number | undefined
  }
}

/**
 * Calls the function with each recorded argument set, then with boundary mutations of them, and
 * applies the crash oracle and the output check to each call.
 */
async function runCorpus(item: Item, options: runCorpus.Options) {
  const { add, fuzzer, seeds, traceFd } = options
  const budget =
    budgets.find((entry) => entry[0].test(item.name))?.[1] ?? mutations
  const random = prng(hash32(item.name))
  const shapes = fuzzer.shapesOf(seeds)
  let calls = 0
  const seen = new Set<string>()
  const attempt = async (mutant: Mutant) => {
    calls++
    if (traceFd !== undefined)
      fs.writeSync(
        traceFd,
        `${JSON.stringify({ harness: 'corpus', input: describe(item, { args: mutant.args, fuzzer }), changes: mutant.changes })}\n`,
      )
    return check(item, { fuzzer, mutant })
  }
  const inputs: Mutant[] = seeds.map((seed) => ({
    args: fuzzer.clone(seed),
    typed: true,
    changes: ['as recorded'],
  }))
  const params = item.params
    .split(',')
    .map((param) => param.trim().match(/^[\w$]+/)?.[0] ?? '')
  for (let i = 0; i < budget; i++) {
    const seed = seeds[i % seeds.length]!
    inputs.push(
      fuzzer.mutate(seed, {
        count: random.next() < 0.25 ? 2 : 1,
        params,
        random,
        shapes,
      }),
    )
  }
  for (const mutant of inputs) {
    const crash = await attempt(mutant)
    if (!crash) continue
    const signature = `${crash.kind}\0${crash.site ?? crash.message}`
    if (seen.has(signature)) continue
    seen.add(signature)
    // Shrinking a hang costs a time limit per try, and the mutations left would likely hang too,
    // so a hang is reported as found and ends the function's fuzzing.
    const hang = crash.kind === 'hang'
    const minimized =
      mutant.typed && !hang
        ? await minimize(item, { crash, fuzzer, mutant, shapes })
        : mutant
    add({
      oracle: crash.oracle,
      harness: 'corpus',
      kind: crash.kind,
      input: `${describe(item, { args: minimized.args, fuzzer })}${mutant.typed ? '' : ' (breaks the declared type)'}`,
      message: crash.message,
      site: crash.site,
    })
    if (hang) break
  }
  return calls
}

declare namespace runCorpus {
  /** The argument sets to call with and mutate, where failures go, and the trace file. */
  type Options = {
    add: (failure: FailureRecord) => void
    fuzzer: Fuzzer
    seeds: readonly (readonly unknown[])[]
    traceFd: number | undefined
  }
}

type Outcome = (Crash & { oracle: string }) | undefined

/**
 * WebAuthn defaults can read the page (`window.location.hostname`, `window.document.title`), and
 * recorded calls can come from browser tests, so the worker stands in for a page at localhost
 * whose title is the package name. Credential prompts return nothing, as a user who dismisses them
 * does.
 */
function stubWindow() {
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: {
      document: { title: library.name },
      location: { hostname: 'localhost', origin: 'http://localhost:5173' },
      navigator: {
        credentials: { create: async () => null, get: async () => null },
      },
    },
  })
}

/** Calls the function once and returns the oracle failure, if any. */
async function check(item: Item, options: check.Options): Promise<Outcome> {
  const { fuzzer, mutant } = options
  const args = fuzzer.clone(mutant.args)
  const size = fuzzer.sizeOf(args)
  const started = performance.now()
  const limit = limitOf(item.name)
  let value: unknown
  try {
    value = Reflect.apply(item.fn, item.self, args)
    if (isThenable(value))
      value = await withTimeout(value, limit ?? asyncTimeoutMs)
  } catch (error) {
    if (error instanceof TimeoutError)
      return {
        oracle: 'crash',
        kind: 'hang',
        message: `did not settle within ${(limit ?? asyncTimeoutMs) / 1000}s`,
      }
    const crash = fuzzer.classify(error, {
      args: mutant.args,
      typed: mutant.typed,
    })
    if (crash && documentedErrors.get(item.name)?.test(crash.kind))
      return undefined
    return crash && { ...crash, oracle: 'crash' }
  }
  const elapsed = performance.now() - started
  if (elapsed > (limit ?? slowCallMs) && size < 10_000)
    return {
      oracle: 'crash',
      kind: 'hang',
      message: `took ${(elapsed / 1000).toFixed(1)}s on a ${size}-character input`,
    }
  // An encoder whose output starts with a prefix the caller picks, such as a Bech32m
  // human-readable part, can return `0x` strings that are not hex.
  if (mutant.typed && !config.freeOutput?.includes(item.name)) {
    const bad = garbageOutput(value, { args: mutant.args, fuzzer })
    if (bad) return { oracle: 'output', kind: 'invalid output', message: bad }
  }
  return undefined
}

declare namespace check {
  /** The input to call with, and the operations that copy and classify it. */
  type Options = { fuzzer: Fuzzer; mutant: Mutant }
}

/**
 * A typed input whose hex strings are all valid should not come back with a hex string that is
 * not hex, or with text such as `NaN`, `undefined`, or `[object Object]` it did not contain.
 */
function garbageOutput(
  value: unknown,
  options: garbageOutput.Options,
): string | undefined {
  const { args, fuzzer } = options
  const inputs = new Set<string>()
  let invalidInput = false
  const collect = (value: unknown, depth: number) => {
    if (depth > 8) return
    if (typeof value === 'string') {
      inputs.add(value)
      if (value.startsWith('0x') && !/^0x[0-9a-fA-F]*$/.test(value))
        invalidInput = true
      if (/NaN|undefined|\[object Object\]|Infinity/.test(value))
        invalidInput = true
    } else if (typeof value === 'number' && !Number.isFinite(value))
      invalidInput = true
    else if (
      value &&
      typeof value === 'object' &&
      !(value instanceof Uint8Array)
    )
      for (const key of Object.keys(value))
        collect((value as Record<string, unknown>)[key], depth + 1)
  }
  collect(args, 0)
  if (invalidInput) return undefined
  let found: string | undefined
  const visit = (value: unknown, at: { depth: number; where: string }) => {
    const { depth, where } = at
    if (found || depth > 6) return
    if (typeof value === 'string') {
      if (inputs.has(value)) return
      // A Bech32 string whose human-readable part is `0x` is not hex, and is not meant to be.
      if (/^0x1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{6,}$/.test(value)) return
      if (value.startsWith('0x') && !/^0x[0-9a-fA-F]*$/.test(value))
        found = `returned ${fuzzer.format(value, 80)}${where ? ` at ${where}` : ''}, which is not hex`
      else if (/^(NaN|undefined|\[object Object\])$|0xNaN|0x-/.test(value))
        found = `returned ${fuzzer.format(value, 80)}${where ? ` at ${where}` : ''}`
    } else if (
      value &&
      typeof value === 'object' &&
      !(value instanceof Uint8Array) &&
      (Array.isArray(value) ||
        Object.getPrototypeOf(value) === Object.prototype)
    )
      for (const key of Object.keys(value).slice(0, 200))
        visit((value as Record<string, unknown>)[key], {
          depth: depth + 1,
          where: where ? `${where}.${key}` : key,
        })
  }
  visit(value, { depth: 0, where: '' })
  return found
}

declare namespace garbageOutput {
  /** The typed input the call returned the value for, and the formatter. */
  type Options = { args: readonly unknown[]; fuzzer: Fuzzer }
}

/**
 * Shrinks a failing typed input while it keeps its type and fails the same way: drops fields
 * other recorded calls leave out, drops array elements, and simplifies numbers and strings.
 */
async function minimize(item: Item, options: minimize.Options) {
  const { crash, fuzzer, mutant, shapes } = options
  const signature = (outcome: Outcome) =>
    outcome && `${outcome.kind}\0${outcome.site ?? outcome.message}`
  const target = `${crash.kind}\0${crash.site ?? crash.message}`
  let best = mutant
  let attempts = 0
  let improved = true
  while (improved && attempts < minimizeAttempts) {
    improved = false
    for (const candidate of simpler(best.args, { fuzzer, shapes })) {
      if (attempts++ >= minimizeAttempts) break
      const next = { ...best, args: candidate }
      if (signature(await check(item, { fuzzer, mutant: next })) === target) {
        best = next
        improved = true
        break
      }
    }
  }
  return best
}

declare namespace minimize {
  /** The failure to keep, the input that caused it, and the recorded shapes. */
  type Options = {
    crash: Crash
    fuzzer: Fuzzer
    mutant: Mutant
    shapes: Shapes
  }
}

/** Simpler variants of an argument set, one change each, that keep the declared types. */
function* simpler(
  args: readonly unknown[],
  options: simpler.Options,
): Generator<readonly unknown[]> {
  const { fuzzer, shapes } = options
  const paths: (readonly (string | number)[])[] = []
  const visit = (value: unknown, path: readonly (string | number)[]) => {
    // A simpler ABI or EIP-712 schema is a different input, not a smaller one.
    if (isSchema(path.at(-1), value)) return
    paths.push(path)
    // A path starts with the argument's index, so the rest of it is the depth.
    const depth = path.length - 1
    if (
      depth > 6 ||
      !value ||
      typeof value !== 'object' ||
      fuzzer.isNamed(value)
    )
      return
    if (value instanceof Uint8Array) return
    for (const key of Object.keys(value))
      visit((value as Record<string, unknown>)[key], [...path, key])
  }
  args.forEach((arg, i) => {
    visit(arg, [i])
  })
  const at = (root: readonly unknown[], path: readonly (string | number)[]) => {
    let parent: unknown = root
    for (const part of path.slice(0, -1))
      parent = (parent as Record<string | number, unknown>)[part]
    return {
      parent: parent as Record<string | number, unknown>,
      key: path.at(-1)!,
    }
  }
  for (const path of paths) {
    const { parent, key } = at(args, path)
    const value = parent[key]
    const variants: unknown[] = []
    if (Array.isArray(value) && value.length > 1 && homogeneous(value))
      variants.push(value.slice(0, value.length >> 1), value.slice(1))
    if (typeof value === 'bigint' && value > 1n) variants.push(1n, value >> 8n)
    if (typeof value === 'number' && Number.isInteger(value) && value > 1)
      variants.push(1, Math.floor(value / 256))
    if (typeof value === 'string' && value.startsWith('0x') && value.length > 4)
      variants.push(
        value.slice(0, Math.max(2, value.length >> 1)),
        `0x${value.slice(4)}`,
      )
    if (
      typeof value === 'string' &&
      !value.startsWith('0x') &&
      value.length > 24
    )
      variants.push(value.slice(0, value.length >> 1))
    if (value instanceof Uint8Array && value.length > 1)
      variants.push(value.slice(0, value.length >> 1))
    for (const variant of variants) {
      const copy = fuzzer.clone(args)
      const { parent } = at(copy, path)
      parent[key] = variant
      yield copy
    }
    if (
      !Array.isArray(parent) &&
      path.length > 1 &&
      value !== undefined &&
      typeof parent === 'object' &&
      keepsType(shapes, {
        args,
        path: path.slice(0, -1),
        record: { ...parent, [key]: undefined },
      })
    ) {
      // Dropping a field another recorded call leaves out keeps the type.
      const copy = fuzzer.clone(args)
      const { parent } = at(copy, path)
      delete parent[key]
      yield copy
    }
  }
}

declare namespace simpler {
  /** The operations that copy the input, and the recorded shapes. */
  type Options = { fuzzer: Fuzzer; shapes: Shapes }
}

function describe(item: Item, options: describe.Options) {
  const name = item.name.split(':')[1]
  return `${name}(${options.args.map((arg) => options.fuzzer.format(arg, 500)).join(', ')})`
}

declare namespace describe {
  /** The arguments of the call, and the formatter. */
  type Options = { args: readonly unknown[]; fuzzer: Fuzzer }
}

type ScriptCoverage = {
  url: string
  functions: readonly {
    functionName: string
    ranges: readonly { startOffset: number; endOffset: number; count: number }[]
  }[]
}

/** A parsed source: its text, the offset of each line, and the range of each function. */
type Source = {
  text: string
  lineStarts: readonly number[]
  functions: readonly (readonly [number, number])[]
}

function sourceOf(sources: Map<string, Source>, url: string) {
  let source = sources.get(url)
  if (source) return source
  const text = fs.readFileSync(new URL(url), 'utf8')
  const lineStarts = [0]
  for (let i = 0; i < text.length; i++)
    if (text.charCodeAt(i) === 10) lineStarts.push(i + 1)
  // Every function's range, so a nested function that never ran counts as not run.
  const functions: [number, number][] = []
  try {
    const ast = acorn.parse(text, {
      ecmaVersion: 'latest',
      sourceType: 'module',
    })
    const visit = (node: unknown) => {
      if (!node || typeof node !== 'object') return
      const record = node as { type?: string; start: number; end: number }
      if (
        record.type === 'FunctionDeclaration' ||
        record.type === 'FunctionExpression' ||
        record.type === 'ArrowFunctionExpression'
      )
        functions.push([record.start, record.end])
      for (const value of Object.values(node))
        if (Array.isArray(value)) value.forEach(visit)
        else if (value && typeof value === 'object' && 'type' in value)
          visit(value)
    }
    visit(ast)
  } catch {}
  source = { text, lineStarts, functions }
  sources.set(url, source)
  return source
}

/**
 * The share of a function's own lines that ran: lines inside its source range, nested functions
 * included, that hold code (not blank, a comment, or only closing brackets), and whose first code
 * character sits in a block that ran. A nested function that never ran is absent from V8's report,
 * so its lines count as not run.
 */
function coverageOf(
  scripts: readonly ScriptCoverage[],
  options: coverageOf.Options,
) {
  const { location, sources } = options
  const source = sourceOf(sources, location.url)
  const offset = source.lineStarts[location.line]! + location.column
  const script = scripts.find((script) => script.url === location.url)
  const ranges = script?.functions.flatMap((fn) => fn.ranges) ?? []
  // The function's own range: the innermost parsed function around its location.
  let own: readonly [number, number] | undefined
  for (const range of source.functions)
    if (
      range[0] <= offset &&
      offset < range[1] &&
      (!own || range[1] - range[0] < own[1] - own[0])
    )
      own = range
  if (!own) return undefined
  const nested = source.functions.filter(
    (range) => range[0] >= own![0] && range[1] <= own![1] && range !== own,
  )
  const countAt = (position: number) => {
    let best: { size: number; count: number } | undefined
    for (const range of ranges)
      if (range.startOffset <= position && position < range.endOffset) {
        const size = range.endOffset - range.startOffset
        if (!best || size <= best.size) best = { size, count: range.count }
      }
    // A nested function V8 did not report never ran.
    for (const range of nested)
      if (range[0] <= position && position < range[1]) {
        const size = range[1] - range[0]
        const reported = ranges.some(
          (r) =>
            r.startOffset <= range[0] + 16 &&
            r.endOffset >= range[1] - 1 &&
            r.endOffset - r.startOffset <= size + 32,
        )
        if (!reported && (!best || size < best.size)) best = { size, count: 0 }
      }
    return best?.count ?? 0
  }
  let lines = 0
  let covered = 0
  let inComment = false
  const firstLine = upperBound(source.lineStarts, own[0]) - 1
  const lastLine = upperBound(source.lineStarts, own[1] - 1) - 1
  for (let line = firstLine; line <= lastLine; line++) {
    const start = Math.max(source.lineStarts[line]!, own[0])
    const end = Math.min(
      (source.lineStarts[line + 1] ?? source.text.length) - 1,
      own[1],
    )
    const text = source.text.slice(start, end)
    let index = 0
    let code = -1
    while (index < text.length) {
      if (inComment) {
        const close = text.indexOf('*/', index)
        if (close < 0) {
          index = text.length
          break
        }
        inComment = false
        index = close + 2
        continue
      }
      const char = text[index]!
      if (/\s/.test(char)) index++
      else if (text.startsWith('//', index)) index = text.length
      else if (text.startsWith('/*', index)) {
        inComment = true
        index += 2
      } else {
        code = index
        break
      }
    }
    if (code < 0) continue
    if (/^[\s})\];,]*$/.test(text.slice(code))) continue
    lines++
    if (countAt(start + code) > 0) covered++
  }
  if (!lines) return undefined
  return {
    percent: Math.round((covered / lines) * 1000) / 10,
    lines,
    covered,
  }
}

declare namespace coverageOf {
  /** The function's location, and the sources parsed so far. */
  type Options = { location: Location; sources: Map<string, Source> }
}

function upperBound(values: readonly number[], target: number) {
  let low = 0
  let high = values.length
  while (low < high) {
    const middle = (low + high) >> 1
    if (values[middle]! <= target) low = middle + 1
    else high = middle
  }
  return low
}

/** A call that reaches for the network throws instead. */
function blockNetwork() {
  const blocked = () => {
    throw new Error('the network is disabled in the fuzzer')
  }
  globalThis.fetch = (async () => blocked()) as unknown as typeof fetch
  // A plain function throws under `new` as the constructor of a class does.
  ;(globalThis as { WebSocket?: unknown }).WebSocket = function WebSocket() {
    blocked()
  }
  const require = nodeModule.createRequire(import.meta.url)
  const net = require('node:net')
  net.connect = net.createConnection = blocked
  const tls = require('node:tls')
  tls.connect = blocked
  for (const name of ['node:http', 'node:https']) {
    const module = require(name)
    module.request = module.get = blocked
  }
  nodeModule.syncBuiltinESMExports()
}

class TimeoutError extends Error {
  override name = 'Fuzz.TimeoutError'
}

function withTimeout<value>(promise: PromiseLike<value>, ms: number) {
  let timer: NodeJS.Timeout | undefined
  return Promise.race([
    promise,
    new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new TimeoutError('timeout')), ms)
    }),
  ]).finally(() => clearTimeout(timer))
}

function isThenable(value: unknown): value is PromiseLike<unknown> {
  return (
    !!value &&
    (typeof value === 'object' || typeof value === 'function') &&
    typeof (value as PromiseLike<unknown>).then === 'function'
  )
}

function isClass(value: unknown) {
  return /^class[\s{]/.test(Function.prototype.toString.call(value))
}

/** A module namespace, or a plain object whose values are all functions or namespaces. */
function isNamespace(value: unknown, depth = 0): value is object {
  if (!value || typeof value !== 'object' || depth > 3) return false
  if (Object.prototype.toString.call(value) === '[object Module]') return true
  const proto = Object.getPrototypeOf(value)
  if (proto !== Object.prototype && proto !== null) return false
  const values = Object.values(value)
  return (
    values.length > 0 &&
    values.every(
      (item) => typeof item === 'function' || isNamespace(item, depth + 1),
    )
  )
}

/** The head of a function's parameter list, from its source. */
function paramsOf(fn: unknown) {
  const text = Function.prototype.toString.call(fn)
  const match =
    text.match(/^(?:async\s+)?(?:function\s*\*?\s*[\w$]*\s*)?\(([^)]*)\)/) ??
    text.match(/^(?:async\s+)?([\w$]+)\s*=>/)
  return match ? match[1]!.replace(/\s+/g, ' ').trim() : ''
}

// Runs last, so every constant above is initialized.
if (mode === 'plan') await plan()
else if (mode === 'fuzz') await fuzz()
else if (mode === 'corpus') await buildCorpus()
else throw new Error(`Unknown mode: ${mode}`)
process.exit(0)
