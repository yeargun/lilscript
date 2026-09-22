#!/usr/bin/env node
// Compare ordinary output and bounded naming search on frozen, already-tested
// workload inputs. All candidates and all objective winners are checked.
import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { performance } from "node:perf_hooks"
import { cpus, loadavg, release } from "node:os"
import { digest, fileIdentity } from "./artifact-evidence.mjs"

const args = process.argv.slice(2)
const flag = (name, fallback) => {
  const at = args.indexOf(name)
  if (at < 0) { assert.notEqual(fallback, undefined, `missing ${name}`); return fallback }
  assert.ok(args[at + 1] && !args[at + 1].startsWith("--"), `missing value for ${name}`)
  return args[at + 1]
}
const out = resolve(flag("--out"))
const compiler = resolve(flag("--compiler"))
const previous = args.includes("--previous-compiler") ? resolve(flag("--previous-compiler")) : null
const codec = resolve(flag("--codec"))
const basePaths = flag("--bases").split(",").map(path => resolve(path))
const trials = Number(flag("--trials", "20"))
const fastOnly = args.includes("--fast-only")
const objectiveStudy = args.includes("--objective-study")
assert.ok(!fastOnly || !objectiveStudy, "fast-only and objective-study are separate experiments")
assert.ok(!fastOnly || previous, "a balanced fast replay requires a valid previous compiler")
assert.ok(!fastOnly || !args.includes("--budgets"), "fast-only replay does not take search budgets")
const budgets = fastOnly ? [] : flag("--budgets", "1,3,8,16").split(",").map(Number)
const byteBudget = Number(flag("--candidate-bytes", "8388608"))
assert.ok(Number.isInteger(trials) && trials >= 4 && trials <= 100)
assert.ok(Number.isSafeInteger(byteBudget) && byteBudget > 0)
assert.ok((fastOnly || budgets.length > 0) && budgets.every((value, i) => Number.isInteger(value)
  && value >= 1 && value <= 512 && (i === 0 || value > budgets[i - 1])))
const bases = basePaths.map(path => ({ path, ...fileIdentity(path), report: JSON.parse(readFileSync(path, "utf8")) }))
const node = { version: process.version, ...fileIdentity(process.execPath) }
const inventory = []
for (const base of bases) {
  assert.equal(base.report.status, "passed")
  assert.equal(base.report.node.sha256, node.sha256)
  assert.deepEqual(base.report.codec, fileIdentity(codec))
  for (const row of base.report.workloads) {
    assert.ok(row.artifacts.every(artifact => artifact.test.exitCode === 0 && artifact.test.stdoutSha256 === row.expected))
    inventory.push({ base, row })
  }
}
assert.equal(new Set(inventory.map(({ row }) => row.name)).size, inventory.length)
const selected = flag("--cases", inventory.map(({ row }) => row.name).join(",")).split(",")
assert.equal(new Set(selected).size, selected.length)
assert.ok(selected.every(name => inventory.some(({ row }) => row.name === name)))
assert.ok(!existsSync(out), "output must be new")
mkdirSync(out, { recursive: true })
const binaries = [[compiler, "compiler"], ...(previous ? [[previous, "previous-compiler"]] : []), [codec, "codec"]]
for (const [source, name] of binaries) cpSync(source, join(out, name))
const tools = dirname(fileURLToPath(import.meta.url))
mkdirSync(join(out, "tool-source"))
const toolSources = ["naming-experiment.mjs", "artifact-evidence.mjs"].map(path => {
  const identity = fileIdentity(join(tools, path))
  cpSync(join(tools, path), join(out, "tool-source", path))
  assert.deepEqual(fileIdentity(join(out, "tool-source", path)), identity)
  return { path, ...identity }
})
const modeSpecs = [
  ...(previous ? [{ name: "previous", binary: "previous-compiler" }] : []), { name: "global", binary: "compiler" },
  ...(fastOnly ? [] : [{ name: "scoped", binary: "compiler" }]),
  ...budgets.flatMap(budget => (objectiveStudy ? ["all", "raw", "gzip", "brotli"] : ["all"]).map(goals => ({
    name: goals === "all" ? `budget-${budget}` : `${goals}-${budget}`, binary: "compiler", budget, goals,
    primary: goals === "all" ? "brotli" : goals,
  }))),
]
const modes = modeSpecs.map(spec => spec.name)
const specs = new Map(modeSpecs.map(spec => [spec.name, spec]))
const report = {
  schemaVersion: 1, status: "running", scope: "bounded-naming-search-experiment",
  createdAt: new Date().toISOString(), trials, budgets, candidateByteBudget: byteBudget, modes, modeSpecs, fastOnly, objectiveStudy,
  command: [process.execPath, ...process.execArgv, ...process.argv.slice(1)],
  compiler: fileIdentity(join(out, "compiler")), previousCompiler: previous ? fileIdentity(join(out, "previous-compiler")) : null,
  codec: fileIdentity(join(out, "codec")), node,
  host: { model: cpus()[0]?.model, cpus: cpus().length, kernel: release(), load: loadavg() },
  bases: bases.map(({ report, ...identity }) => identity),
  cases: { available: inventory.map(({ row }) => row.name), selected },
  tools: toolSources, workloads: [],
}
const save = () => writeFileSync(join(out, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
const environment = { ...process.env }
delete environment.NODE_OPTIONS
delete environment.NODE_TEST_CONTEXT
const run = (command, args) => {
  const result = spawnSync(command, args, { env: environment, encoding: "utf8", timeout: 120000, maxBuffer: 1 << 28 })
  assert.equal(result.status, 0, `${command} ${args.join(" ")}\n${result.stderr}\n${result.error ?? ""}`)
  return result.stdout
}
const searchSignature = search => search == null ? null : {
  ...search, candidates: search.candidates.map(({ path, ...candidate }) => candidate),
}
save()
try {
  for (const { base, row: original } of inventory.filter(({ row }) => selected.includes(row.name))) {
    const directory = join(out, original.name)
    mkdirSync(directory)
    const source = join(directory, "source.lil")
    const setup = join(directory, "setup.mjs")
    cpSync(join(dirname(base.path), original.name, "source.lil"), source)
    cpSync(join(dirname(base.path), original.name, "setup.mjs"), setup)
    assert.deepEqual(fileIdentity(source), original.input)
    assert.deepEqual(fileIdentity(setup), original.setup)
    const row = { name: original.name, input: original.input, setup: original.setup, expected: original.expected,
      contract: original.contract, provenance: original.provenance,
      referenceAttempts: original.referenceAttempts,
      compiles: [], archives: [], artifacts: [], objectives: {}, references: [] }
    report.workloads.push(row)
    const stable = new Map()
    const compile = (mode, stem, archive = false) => {
      const spec = specs.get(mode)
      const budget = spec.budget
      const compiler = join(out, spec.binary)
      const settings = budget ? ["--search-plans", String(budget), "--search-bytes", String(byteBudget),
        "--objective", spec.primary, ...(spec.goals === "all" ? ["--search-all-objectives"] : [])]
        : mode === "scoped" ? ["--name-style", "scoped"] : []
      if (archive) settings.push("--search-artifacts", `${stem}-artifacts`)
      const started = performance.now()
      const code = run("/usr/bin/time", ["-f", "%M", "-o", `${stem}.rss`, compiler, source,
        "--analysis", "regions", ...settings, "--metrics", `${stem}.json`])
      const elapsedMs = performance.now() - started
      const metrics = JSON.parse(readFileSync(`${stem}.json`, "utf8"))
      const signature = { sha256: digest(code), rounds: metrics.rounds, target: metrics.targetChoices,
        output: metrics.outputPreparation, search: searchSignature(metrics.search) }
      if (stable.has(mode)) assert.deepEqual(signature, stable.get(mode), "non-deterministic output or work")
      else stable.set(mode, signature)
      if (mode !== "previous") assert.equal(metrics.outputPreparation.namingBases, 1)
      if (budget) {
        assert.ok([3, 4].includes(metrics.schemaVersion), "requested-objective measurement contract")
        if (metrics.schemaVersion === 4) {
          assert.equal(metrics.timingScope, "compiler stages exclude diagnostic serialization and artifact archive I/O")
        }
        assert.ok(metrics.search.proposalSteps <= budget)
        assert.ok(metrics.search.retainedBytes <= byteBudget)
        const requested = spec.goals === "all" ? ["raw", "gzip", "brotli"] : [spec.goals]
        assert.deepEqual(metrics.search.objectives, requested)
        const codecs = requested.filter(goal => goal !== "raw").length
        assert.equal(metrics.search.measurementCalls, codecs * metrics.search.candidates.length)
        for (const candidate of metrics.search.candidates) {
          assert.ok(Number.isSafeInteger(candidate.sizes.raw) && candidate.sizes.raw >= 0)
          assert.equal(candidate.sizes.gzip9 == null, !requested.includes("gzip"))
          assert.equal(candidate.sizes.brotli11 == null, !requested.includes("brotli"))
        }
        for (const [index, goal] of ["raw", "gzip", "brotli"].entries()) {
          assert.equal(metrics.search.winners[index] != null, requested.includes(goal))
        }
      }
      return { code, mode, elapsedMs, peakRssKiB: Number(readFileSync(`${stem}.rss`, "utf8").trim()), metrics }
    }
    const ordinary = new Map()
    for (let trial = 0; trial < trials; trial++) {
      for (let offset = 0; offset < modes.length; offset++) {
        const mode = modes[(offset + trial) % modes.length]
        const { code, ...result } = compile(mode, join(directory, `${mode}-${trial}`))
        row.compiles.push({ ...result, trial, sha256: digest(code) })
        if (!specs.get(mode).budget && !ordinary.has(mode)) ordinary.set(mode, code)
      }
    }
    const addArtifact = (name, path, expectedSizes = null) => {
      row.artifacts.push({ name, path, ...fileIdentity(path), expectedSizes })
    }
    for (const [mode, code] of ordinary) {
      const path = join(directory, `${mode}.mjs`)
      writeFileSync(path, code); addArtifact(mode, path)
    }
    for (const spec of modeSpecs.filter(spec => spec.budget)) {
      const mode = spec.name
      const { code, ...archive } = compile(mode, join(directory, `${mode}-archive`), true)
      row.archives.push(archive)
      const search = archive.metrics.search
      search.candidates.forEach((candidate, index) => {
        assert.equal(fileIdentity(candidate.path).sha256, candidate.sha256)
        addArtifact(`${mode}-${index}`, candidate.path, candidate.sizes)
      })
      const primary = ["raw", "gzip", "brotli"].indexOf(spec.primary)
      assert.equal(digest(code), search.candidates[search.winners[primary]].sha256)
      row.objectives[mode] = search.winners.map(index => index == null ? null : search.candidates[index].sha256)
      for (const [index, key] of ["raw", "gzip9", "brotli11"].entries()) {
        const winner = search.winners[index]
        if (winner == null) continue
        assert.equal(search.candidates[winner].sizes[key], Math.min(...search.candidates.map(candidate => candidate.sizes[key])))
      }
    }
    for (const goals of objectiveStudy ? ["all", "raw", "gzip", "brotli"] : ["all"]) {
      const family = modeSpecs.filter(spec => spec.goals === goals)
      for (let index = 1; index < family.length; index++) {
        const previous = stable.get(family[index - 1].name).search
        const current = stable.get(family[index].name).search
        assert.deepEqual(current.attempts.slice(0, previous.attempts.length), previous.attempts, "work prefix changed")
        for (const [metric, key] of ["raw", "gzip9", "brotli11"].entries()) {
          if (current.winners[metric] == null) { assert.equal(previous.winners[metric], null); continue }
          assert.ok(current.candidates[current.winners[metric]].sizes[key] <= previous.candidates[previous.winners[metric]].sizes[key])
        }
      }
    }
    for (const artifact of original.artifacts.filter(artifact => artifact.name.startsWith("terser-"))) {
      const path = join(directory, `reference-${artifact.name}.mjs`)
      cpSync(artifact.path, path)
      assert.equal(fileIdentity(path).sha256, artifact.sha256)
      addArtifact(`reference-${artifact.name}`, path,
        Object.fromEntries(["raw", "gzip9", "brotli11"].map(metric => [metric, artifact[metric]])))
      row.references.push({ name: artifact.name, options: artifact.options, sourceBase: base.path })
    }
    const distinct = new Map()
    for (const artifact of row.artifacts) {
      if (distinct.has(artifact.sha256)) {
        assert.deepEqual(readFileSync(artifact.path), readFileSync(distinct.get(artifact.sha256).path))
      } else {
        const result = run(process.execPath, ["--import", pathToFileURL(setup).href, artifact.path])
        assert.equal(digest(result), row.expected, `${row.name}/${artifact.name}`)
        distinct.set(artifact.sha256, { path: artifact.path, test: { exitCode: 0, stdoutSha256: row.expected } })
      }
      artifact.test = distinct.get(artifact.sha256).test
    }
    const measured = JSON.parse(run(join(out, "codec"), ["--json", ...[...distinct.values()].map(row => row.path)]))
    row.codecs = measured.codecs
    for (const artifact of row.artifacts) {
      const sizes = measured.artifacts.find(result => result.path === distinct.get(artifact.sha256).path)
      assert.ok(sizes)
      for (const metric of ["raw", "gzip9", "brotli11"]) {
        if (artifact.expectedSizes?.[metric] != null) assert.equal(sizes[metric], artifact.expectedSizes[metric], "compiler/external codec scores differ")
        artifact[metric] = sizes[metric]
      }
      assert.equal(fileIdentity(artifact.path).sha256, artifact.sha256)
    }
    row.distinctTestedArtifacts = distinct.size
    row.distinctCodecEvaluations = distinct.size
    assert.deepEqual(fileIdentity(source), row.input)
    assert.deepEqual(fileIdentity(setup), row.setup)
    save()
    process.stderr.write(`${row.name}: ${row.compiles.length} paired compiles; ${distinct.size} distinct tested artifacts; all codec winners checked\n`)
  }
  for (const { report, path, ...identity } of bases) assert.deepEqual(fileIdentity(path), identity)
  for (const { path, ...identity } of toolSources) {
    assert.deepEqual(fileIdentity(join(tools, path)), identity)
    assert.deepEqual(fileIdentity(join(out, "tool-source", path)), identity)
  }
  for (const [, name] of binaries) {
    assert.deepEqual(fileIdentity(join(out, name)), report[name === "previous-compiler" ? "previousCompiler" : name])
  }
  report.status = "passed"
} catch (error) {
  report.status = "failed"; report.error = String(error.stack ?? error); throw error
} finally { save() }
