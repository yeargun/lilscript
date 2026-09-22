#!/usr/bin/env node
// A controlled analysis experiment. This does not certify a library, an SSA
// architecture, competitive completion, or a production compiler speedup.
import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { cpus, loadavg, release } from "node:os"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { performance } from "node:perf_hooks"
import { architectureFixtures, architectureFixtureFiles } from "./architecture-fixtures.mjs"
import { digest, fileIdentity, fingerprint } from "./artifact-evidence.mjs"

const args = process.argv.slice(2)
const flag = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback
const out = resolve(flag("--out", "/missing-experiment-output"))
const compiler = resolve(flag("--compiler", "/missing-experiment-compiler"))
const codec = resolve(flag("--codec", "/missing-experiment-codec"))
const referenceRoot = resolve(flag("--references", "/missing-pinned-references"))
const previousCompiler = args.includes("--previous-compiler") ? resolve(flag("--previous-compiler")) : null
const previousAnalyses = previousCompiler ? flag("--previous-analyses", "indexed").split(",") : []
assert.ok(previousCompiler || !args.includes("--previous-analyses"), "previous analyses require a previous compiler")
assert.ok(new Set(previousAnalyses).size === previousAnalyses.length
  && previousAnalyses.every(mode => ["none", "tree", "indexed", "memoized", "regions", "values"].includes(mode)), "invalid previous analysis sweep")
const compareOwnedData = args.includes("--compare-owned-data")
const compareRegionValues = args.includes("--compare-region-values")
const trials = Number(flag("--trials", "15"))
const terserPasses = flag("--terser-passes", "1,3").split(",").map(Number)
const referenceProfiles = {
  standard: {}, "no-side-effects": { side_effects: false }, "keep-unused": { unused: false },
  "preserve-effects": { side_effects: false, unused: false }, "no-evaluate": { evaluate: false },
  "mangle-only": null,
}
const terserProfiles = flag("--terser-profiles", "standard").split(",")
assert.ok(terserProfiles.length && new Set(terserProfiles).size === terserProfiles.length
  && terserProfiles.every(profile => Object.hasOwn(referenceProfiles, profile)), "invalid reference profiles")
assert.ok(Number.isInteger(trials) && trials >= 4 && trials <= 100)
assert.ok(terserPasses.length > 0 && new Set(terserPasses).size === terserPasses.length
  && terserPasses.every(value => Number.isInteger(value) && value >= 1 && value <= 20), "invalid Terser pass sweep")
const inventory = architectureFixtures()
const availableCases = inventory.map(fixture => fixture.name)
assert.equal(new Set(availableCases).size, availableCases.length, "duplicate fixture name")
const selectedCases = args.includes("--cases") ? flag("--cases").split(",") : availableCases
assert.ok(selectedCases.length > 0 && new Set(selectedCases).size === selectedCases.length
  && selectedCases.every(name => availableCases.includes(name)), "invalid explicit case inventory")
const fixtures = inventory.filter(fixture => selectedCases.includes(fixture.name))
assert.ok(!existsSync(out), "experiment output must be new")
mkdirSync(out, { recursive: true })
const tools = dirname(fileURLToPath(import.meta.url))
const snapshotTree = (root) => {
  const files = []
  function walk(part) {
    for (const name of readdirSync(join(root, part)).sort()) {
      const path = join(part, name)
      const stat = lstatSync(join(root, path))
      assert.ok(!stat.isSymbolicLink(), `pinned reference contains a symlink: ${path}`)
      if (stat.isDirectory()) walk(path)
      else files.push({ path, ...fileIdentity(join(root, path)) })
    }
  }
  walk("")
  return { sha256: fingerprint(files), files }
}
cpSync(compiler, join(out, "compiler"))
cpSync(codec, join(out, "codec"))
if (previousCompiler) cpSync(previousCompiler, join(out, "previous-compiler"))
const references = snapshotTree(referenceRoot)
const environment = { ...process.env }
delete environment.NODE_OPTIONS
delete environment.NODE_TEST_CONTEXT
const report = {
  schemaVersion: 1, scope: "closed-microprogram-analysis-experiment", status: "running",
  createdAt: new Date().toISOString(), trials, terserPasses, terserProfiles,
  compiler: fileIdentity(join(out, "compiler")), codec: fileIdentity(join(out, "codec")),
  previousCompiler: previousCompiler ? fileIdentity(join(out, "previous-compiler")) : null,
  previousAnalyses,
  cases: { available: availableCases, selected: fixtures.map(fixture => fixture.name) },
  command: [process.execPath, ...process.execArgv, ...process.argv.slice(1)], compareOwnedData, compareRegionValues,
  node: { version: process.version, execArgv: process.execArgv, ...fileIdentity(process.execPath) },
  host: { model: cpus()[0]?.model, cpus: cpus().length, kernel: release(), load: loadavg() },
  tools: ["architecture-experiment.mjs", "architecture-fixtures.mjs", "artifact-evidence.mjs", ...architectureFixtureFiles].map(path => ({ path, ...fileIdentity(join(tools, path)) })),
  references, workloads: [],
}
for (const { path, ...identity } of report.tools) {
  const archived = join(out, "tool-source", path)
  mkdirSync(dirname(archived), { recursive: true })
  cpSync(join(tools, path), archived)
  assert.deepEqual(fileIdentity(archived), identity, "tool changed while being archived")
}
const save = () => writeFileSync(join(out, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
save()
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", env: environment, timeout: 120000, maxBuffer: 1 << 28, ...options })
  assert.equal(result.status, 0, `${command} ${args.join(" ")}\n${result.stderr}\n${result.error ?? ""}`)
  return result
}
const modes = ["none", "tree", "indexed", "memoized", "indexed-fresh"]
for (const analysis of previousAnalyses) modes.push(`previous-${analysis}`)
if (compareOwnedData) modes.push("indexed-arrays")
if (compareRegionValues) modes.push("regions", "values")
try {
  const { minify } = await import(pathToFileURL(join(referenceRoot, "node_modules/terser/main.js")))
  report.terserVersion = JSON.parse(readFileSync(join(referenceRoot, "node_modules/terser/package.json"))).version
  for (const fixture of fixtures) {
    const directory = join(out, fixture.name)
    mkdirSync(directory)
    const input = join(directory, "source.lil")
    writeFileSync(input, fixture.source)
    writeFileSync(join(directory, "expected.txt"), fixture.expected)
    const setup = join(directory, "setup.mjs")
    writeFileSync(setup, fixture.setup ?? "")
    const row = { name: fixture.name, provenance: fixture.provenance, contract: fixture.contract, input: fileIdentity(input), expected: digest(fixture.expected), setup: fileIdentity(setup), compiles: [], artifacts: [], comparisons: {} }
    report.workloads.push(row)
    const armBytes = new Map()
    // Rotate all arms through each order position. Each invocation has a fresh
    // process; this does not claim a cold filesystem cache or installed build.
    for (let trial = 0; trial < trials; trial++) {
      const order = modes.map((_, index) => modes[(index + trial) % modes.length])
      for (const mode of order) {
        const stem = join(directory, `${mode}-${trial}`)
        const started = performance.now()
        const previous = mode.startsWith("previous-")
        const analysis = previous ? mode.slice("previous-".length)
          : ["indexed-fresh", "indexed-arrays"].includes(mode) ? "indexed" : mode
        const selectedCompiler = join(out, previous ? "previous-compiler" : "compiler")
        const dataArguments = mode === "indexed-arrays" ? ["--owned-data", "arrays"] : []
        const result = run("/usr/bin/time", ["-f", "%M", "-o", `${stem}.rss`, selectedCompiler, input, "--analysis", analysis, "--fact-reuse", mode === "indexed-fresh" ? "none" : "unchanged", ...dataArguments, "--metrics", `${stem}.json`])
        const elapsedMs = performance.now() - started
        const identity = digest(result.stdout)
        if (armBytes.has(mode)) assert.equal(identity, digest(armBytes.get(mode)), "repeated compilation changed bytes")
        else armBytes.set(mode, result.stdout)
        row.compiles.push({ mode, trial, elapsedMs, peakRssKiB: Number(readFileSync(`${stem}.rss`, "utf8").trim()), sha256: identity, metrics: JSON.parse(readFileSync(`${stem}.json`, "utf8")) })
      }
    }
    assert.equal(armBytes.get("tree"), armBytes.get("indexed"), "different transforms invalidate the analysis comparison")
    assert.equal(armBytes.get("tree"), armBytes.get("memoized"), "different transforms invalidate the analysis comparison")
    assert.equal(armBytes.get("indexed"), armBytes.get("indexed-fresh"), "fact reuse must preserve selected output")
    if (compareRegionValues) assert.equal(armBytes.get("regions"), armBytes.get("values"), "region/value strategies must use identical transforms and facts")
    const finalists = [...armBytes].map(([name, source]) => ({ name, source }))
    row.referenceAttempts = []
    for (const profile of terserProfiles) for (const passes of profile === "mangle-only" ? [null] : terserPasses) {
      const options = { module: true, ecma: 2022,
        compress: profile === "mangle-only" ? false : { passes, ...referenceProfiles[profile] },
        mangle: true, format: { comments: false } }
      if (fixture.contract?.observableFunctionNames) options.keep_fnames = true
      const name = profile === "standard" ? `terser-${passes}` : `terser-${profile}${passes == null ? "" : `-${passes}`}`
      const attempt = { name, profile, options, status: "running" }
      row.referenceAttempts.push(attempt)
      save()
      const started = performance.now()
      try {
        const result = await minify(armBytes.get("none"), options)
        attempt.status = "emitted"
        attempt.minifyMs = performance.now() - started
        finalists.push({ name, source: result.code, options, minifyMs: attempt.minifyMs, reference: attempt })
      } catch (error) {
        attempt.status = "failed"
        attempt.error = String(error.stack ?? error)
      }
    }
    for (const artifact of finalists) {
      const path = join(directory, `${artifact.name}.mjs`)
      writeFileSync(path, artifact.source)
      const identity = fileIdentity(path)
      const result = spawnSync(process.execPath, ["--import", pathToFileURL(setup).href, path],
        { encoding: "utf8", env: environment, timeout: 120000, maxBuffer: 1 << 28 })
      const test = { exitCode: result.status, signal: result.signal,
        stdoutSha256: digest(result.stdout ?? ""), stderrSha256: digest(result.stderr ?? ""), error: result.error?.message }
      assert.deepEqual(fileIdentity(path), identity, "tested artifact changed")
      const valid = result.status === 0 && !result.error && result.stdout === fixture.expected
      if (artifact.reference) {
        Object.assign(artifact.reference, { status: valid ? "passed" : "behavior-failed", artifact: { path, ...identity }, test })
        if (!valid) {
          // A rejected reference remains reviewable and is never a size floor.
          artifact.reference.stdout = result.stdout ?? ""
          artifact.reference.stderr = result.stderr ?? ""
          save()
          continue
        }
      }
      assert.ok(valid, `behavior differs: ${fixture.name}/${artifact.name}\n${JSON.stringify(test)}\n${result.stdout}\n${result.stderr}`)
      row.artifacts.push({ name: artifact.name, path, ...identity, options: artifact.options, minifyMs: artifact.minifyMs, test })
    }
    row.referenceCoverage = {
      eligible: row.referenceAttempts.filter(attempt => attempt.status === "passed").length,
      rejected: row.referenceAttempts.filter(attempt => attempt.status !== "passed").length,
    }
    // Complete-artifact identity permits exact reuse across analysis arms.
    // Each distinct byte sequence needs one codec evaluation, not one per label.
    const distinct = new Map(row.artifacts.map(artifact => [artifact.sha256, artifact]))
    const sizes = JSON.parse(run(join(out, "codec"), ["--json", ...[...distinct.values()].map(artifact => artifact.path)]).stdout)
    row.codecs = sizes.codecs
    row.codecEvaluations = distinct.size
    for (const artifact of row.artifacts) {
      const measured = distinct.get(artifact.sha256)
      const size = sizes.artifacts.find(size => size.path === measured.path)
      assert.ok(size, "missing complete-artifact codec result")
      Object.assign(artifact, { raw: size.raw, gzip9: size.gzip9, brotli11: size.brotli11 })
    }
    assert.deepEqual(fileIdentity(input), row.input, "input changed during trials")
    assert.deepEqual(fileIdentity(setup), row.setup, "runtime setup changed during trials")
    save()
    process.stderr.write(`${fixture.name}: ${trials * modes.length} compiles; ${row.artifacts.length} valid artifacts; ${row.referenceCoverage.rejected} references rejected\n`)
  }
  assert.deepEqual(snapshotTree(referenceRoot), references, "reference toolchain changed")
  assert.deepEqual(fileIdentity(join(out, "compiler")), report.compiler)
  if (previousCompiler) assert.deepEqual(fileIdentity(join(out, "previous-compiler")), report.previousCompiler)
  assert.deepEqual(fileIdentity(join(out, "codec")), report.codec)
  for (const { path, ...identity } of report.tools) {
    assert.deepEqual(fileIdentity(join(tools, path)), identity, "experiment tool changed during the run")
    assert.deepEqual(fileIdentity(join(out, "tool-source", path)), identity, "archived tool changed during the run")
  }
  report.status = "passed"
} catch (error) {
  report.status = "failed"
  report.error = String(error.stack ?? error)
  throw error
} finally { save() }
