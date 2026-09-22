#!/usr/bin/env node
// Supplemental references over an immutable completed experiment. This does
// not recompile candidates, measure compilation speed or certify library gates.
import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { digest, fileIdentity } from "./artifact-evidence.mjs"

const args = process.argv.slice(2)
const flag = name => {
  const at = args.indexOf(name)
  assert.ok(at >= 0 && args[at + 1], `missing ${name}`)
  return args[at + 1]
}
const input = resolve(flag("--experiment"))
const references = resolve(flag("--references"))
const out = resolve(flag("--out"))
const base = JSON.parse(readFileSync(input, "utf8"))
assert.equal(base.status, "passed", "the base experiment must be complete and valid")
assert.equal(base.scope, "closed-microprogram-analysis-experiment")
assert.ok(!existsSync(out), "reference output must be new")
mkdirSync(out, { recursive: true })
const inventory = (root, part = "") => readdirSync(join(root, part)).sort().flatMap(name => {
  const path = join(part, name)
  const stat = lstatSync(join(root, path))
  assert.ok(!stat.isSymbolicLink(), `reference symlink: ${path}`)
  return stat.isDirectory() ? inventory(root, path) : [{ path, ...fileIdentity(join(root, path)) }]
})
assert.deepEqual(inventory(references), base.references.files, "reference closure differs")
const codec = join(out, "codec")
cpSync(join(dirname(input), "codec"), codec)
assert.deepEqual(fileIdentity(codec), base.codec)
const report = {
  schemaVersion: 1, status: "running", createdAt: new Date().toISOString(),
  scope: "conditional-pristine-builtin-microprogram-reference",
  contract: {
    requires: "Standard ECMAScript builtin functions, classes and prototype methods are not altered or replaced.",
    coverage: "The declared closed fixture setups satisfy this additional condition. Every emitted reference is executed against the same cases.",
    limitation: "This conditional floor does not replace the general host contract or establish eligibility for an open library.",
    unsafeMath: false, unsafeComparisons: false, unsafeMethods: false,
  },
  base: { path: input, ...fileIdentity(input) }, codec: base.codec,
  command: [process.execPath, ...process.execArgv, ...process.argv.slice(1)],
  node: { version: process.version, ...fileIdentity(process.execPath) },
  terserVersion: base.terserVersion, references: base.references, workloads: [],
}
const tools = dirname(fileURLToPath(import.meta.url))
report.tools = ["architecture-reference-sweep.mjs", "artifact-evidence.mjs"].map(path => {
  const identity = fileIdentity(join(tools, path))
  mkdirSync(join(out, "tool-source"), { recursive: true })
  cpSync(join(tools, path), join(out, "tool-source", path))
  assert.deepEqual(fileIdentity(join(out, "tool-source", path)), identity)
  return { path, ...identity }
})
const save = () => writeFileSync(join(out, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
const environment = { ...process.env }
delete environment.NODE_OPTIONS
delete environment.NODE_TEST_CONTEXT
const run = (command, args) => {
  const result = spawnSync(command, args, { env: environment, encoding: "utf8", timeout: 120000, maxBuffer: 1 << 28 })
  assert.equal(result.status, 0, `${command}: ${result.stderr}\n${result.error ?? ""}`)
  return result.stdout
}
save()
try {
  const { minify } = await import(pathToFileURL(join(references, "node_modules/terser/main.js")))
  for (const workload of base.workloads) {
    const direct = workload.artifacts.find(row => row.name === "none")
    const candidate = workload.artifacts.find(row => row.name === "regions")
    assert.ok(direct && candidate)
    const directory = join(out, workload.name)
    mkdirSync(directory)
    const source = join(directory, "input.mjs")
    const setup = join(directory, "setup.mjs")
    cpSync(direct.path, source)
    cpSync(join(dirname(direct.path), "setup.mjs"), setup)
    assert.deepEqual(fileIdentity(source), { sha256: direct.sha256, bytes: direct.bytes })
    assert.deepEqual(fileIdentity(setup), workload.setup)
    const row = { case: workload.name, contract: workload.contract, source: fileIdentity(source), setup: workload.setup, expected: workload.expected, attempts: [], artifacts: [] }
    report.workloads.push(row)
    for (const passes of base.terserPasses) {
      const options = { module: true, ecma: 2022, compress: { passes, unsafe: true, builtins_ecma: 2022, builtins_pure: true, unsafe_math: false, unsafe_comps: false, unsafe_methods: false }, mangle: true, format: { comments: false } }
      if (workload.contract?.observableFunctionNames) options.keep_fnames = true
      const attempt = { name: `terser-pristine-${passes}`, options, status: "running" }
      row.attempts.push(attempt)
      save()
      const result = await minify(readFileSync(source, "utf8"), options)
      const path = join(directory, `${attempt.name}.mjs`)
      writeFileSync(path, result.code)
      const identity = fileIdentity(path)
      assert.equal(digest(run(process.execPath, ["--import", pathToFileURL(setup).href, path])), workload.expected, `${workload.name}/${attempt.name}`)
      assert.deepEqual(fileIdentity(path), identity)
      attempt.status = "passed"
      row.artifacts.push({ name: attempt.name, path, ...identity, test: { exitCode: 0, stdoutSha256: workload.expected } })
    }
    const distinct = new Map(row.artifacts.map(row => [row.sha256, row.path]))
    const sizes = JSON.parse(run(codec, ["--json", ...distinct.values()])).artifacts
    for (const artifact of row.artifacts) {
      const measured = sizes.find(size => size.path === distinct.get(artifact.sha256))
      assert.ok(measured)
      for (const metric of ["raw", "gzip9", "brotli11"]) artifact[metric] = measured[metric]
    }
    row.codecEvaluations = distinct.size
    row.candidate = Object.fromEntries(["sha256", "raw", "gzip9", "brotli11"].map(key => [key, candidate[key]]))
    for (const [label, artifacts] of [["safeMinimum", workload.artifacts.filter(row => row.name.startsWith("terser-"))], ["pristineMinimum", row.artifacts]]) {
      assert.ok(artifacts.length)
      row[label] = Object.fromEntries(["raw", "gzip9", "brotli11"].map(metric => [metric, Math.min(...artifacts.map(artifact => artifact[metric]))]))
    }
    assert.deepEqual(fileIdentity(source), row.source)
    assert.deepEqual(fileIdentity(setup), row.setup)
    save()
    process.stderr.write(`${row.case}: ${row.artifacts.length} conditional references pass\n`)
  }
  assert.deepEqual(fileIdentity(input), { sha256: report.base.sha256, bytes: report.base.bytes })
  assert.deepEqual(inventory(references), base.references.files)
  assert.deepEqual(fileIdentity(codec), report.codec)
  for (const { path, ...identity } of report.tools) {
    assert.deepEqual(fileIdentity(join(tools, path)), identity)
    assert.deepEqual(fileIdentity(join(out, "tool-source", path)), identity)
  }
  report.status = "passed"
} catch (error) {
  report.status = "failed"
  report.error = String(error.stack ?? error)
  throw error
} finally { save() }
