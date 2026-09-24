#!/usr/bin/env node
// SUPERSEDED by scripts/ports.mjs (plan task M2.6): one versioned port runner with
// an expected-failure ledger (tests/ports/expected-failures.json). See
// docs/testing.md. Kept for history; do not extend.
//
// portgate — make the library ports gate the compiler.
//
// WHY THIS EXISTS
//
// As of 2026-09-04 nothing in this repository routinely proved that a compiler
// change preserves the behaviour of the shipped libraries:
//
//   * scripts/release-check.sh runs 74 stages and none of them is a port suite.
//   * 18 of 25 ports' `npm test` never rebuilds, so it validates a committed
//     dist/ produced by some earlier binary -- not the one under test.
//   * mobxlil's suite builds `--dev`; zodlil's builds lilscript.dev.toml at
//     optimization_level 8, below ParsedPeephole::minimum_level() == 9, so the
//     entire fold layer is switched off for the port whose fold miscompile
//     forced that config in the first place.
//   * katexlil's build script skips compilation on an mtime cache unless
//     `--force`, which once published a false "byte-identical" fleet row.
//
// Three fold miscompiles reached shipped artifacts through that gap. This tool
// closes it: it rebuilds every port from source with a NAMED compiler binary,
// refuses to believe a build that did not happen, records the failing-test SET
// rather than a pass/fail bit, and diffs two arms.
//
// USAGE
//
//   node finer/tools/portgate.mjs record  --arm baseline --compiler target/release/lilscript
//   node finer/tools/portgate.mjs record  --arm candidate --compiler /tmp/arms/cand/lilscript
//   node finer/tools/portgate.mjs compare finer/out/portgate/baseline finer/out/portgate/candidate
//
//   --ports a,b      restrict to named ports (default: every sibling *lil with a build script)
//   --out DIR        record destination (default finer/out/portgate/<arm>)
//   --skip-tests     build and measure only; useful while iterating on the compiler
//   --timeout S      default per-phase timeout, seconds (default 2700)
//   --build-timeout S / --test-timeout S / --codec-timeout S override one phase
//
// Builds run in isolated arm workspaces through a receipt-producing compiler
// wrapper. Elapsed time and timing text are never proof that compilation ran.
// A recorded build without a certified production test adapter remains
// diagnostic evidence. `compare` cannot promote an unverified test row.
//
// This measures on THIS host. Per objective.md §9 the builds belong on the
// pool; this tool is the per-port contract that finer/tools/fleet.mjs should
// dispatch, not a replacement for it.

import { execFileSync, spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs"
import { dirname, join, relative, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { fileIdentity, fingerprint, snapshotInputs, validateBuild, validateMeasurements, validateTests } from "./artifact-evidence.mjs"
import { runNodeTestEvidence } from "./node-test-evidence.mjs"
import { runVitestTestEvidence } from "./vitest-test-evidence.mjs"
import { runBoundedCommand } from "./bounded-command.mjs"

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..")
const siblings = resolve(repo, "..")
const codec = join(repo, "target/release/lilscript-codec")

const argv = process.argv.slice(2)
const command = argv[0]
const flag = (name, fallback) => {
  const at = argv.indexOf(`--${name}`)
  return at === -1 ? fallback : argv[at + 1]
}
const has = (name) => argv.includes(`--${name}`)
const workloadPath = resolve(flag("manifest", join(repo, "benchmarks/libraries/maintained-workloads.json")))
const workloadDocument = existsSync(workloadPath) ? JSON.parse(readFileSync(workloadPath, "utf8")) : null
const workloadEntries = [...(workloadDocument?.libraries ?? []), ...(workloadDocument?.additionalCoverage ?? [])]
const workspaceRoot = flag("workspaces-root", null)
function workloadFor(port) { return workloadEntries.find((row) => row.id === port) }
function sourceWorkspace(port) { return workspaceRoot ? resolve(workspaceRoot, port) : resolve(workloadFor(port)?.workspace ?? join(siblings, port)) }

const log = (line) => process.stderr.write(`[portgate] ${line}\n`)
const die = (message) => {
  throw new Error(message)
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex")
}

function discoverPorts() {
  if (workloadDocument) return workloadEntries.filter((row) => row.required && row.kind !== "related-checkout").map((row) => row.id).sort()
  return readdirSync(siblings)
    .filter((name) => name.endsWith("lil") || name.startsWith("lil-"))
    .filter((name) => existsSync(join(siblings, name, "scripts/build.mjs")))
    .sort()
}

// Measure exact file bytes with the pinned encoders. Node's own zlib/brotli
// disagreed with these on 96 of 279 artifacts (objective.md §8), so nothing
// else may produce a published size.
async function measure(paths, pinnedCodec, timeoutMs) {
  if (!existsSync(pinnedCodec)) die(`${pinnedCodec} is missing -- cargo build --release --bin lilscript-codec`)
  const result = await runBoundedCommand(pinnedCodec, ["--json", ...paths], { encoding: "utf8", timeoutMs, maxBuffer: 1 << 28 })
  const evidence = { status: result.status, signal: result.signal, error: result.error?.message ?? null, supervision: result.supervision }
  if (result.status !== 0 || result.error || result.signal) return { evidence, error: "canonical codec command did not complete" }
  try {
    return { evidence, sizes: Object.fromEntries(validateMeasurements(JSON.parse(result.stdout), paths)) }
  } catch (error) {
    return { evidence, error: `invalid canonical codec measurements: ${error.message}` }
  }
}

function portArtifacts(workspace) {
  const dist = join(workspace, "dist")
  if (!existsSync(dist)) return []
  const found = []
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name)
      if (entry.isDirectory()) walk(path)
      else if (/\.(js|mjs|cjs)$/.test(entry.name) && !entry.name.endsWith(".map")) found.push(path)
    }
  }
  walk(dist)
  return found.sort()
}

// git state of the port checkout, so a comparison can say whether it is even
// looking at the same source. A dirty port is not disqualifying -- most are
// mid-migration -- but a comparison across two different dirty states is not a
// compiler measurement and must be reported as such.
function portSource(cwd) {
  const git = (args) => {
    const r = spawnSync("git", args, { cwd, encoding: "utf8" })
    return r.status === 0 ? r.stdout.trim() : null
  }
  const dirty = git(["status", "--porcelain", "--", "src", "scripts", "package.json"])
  return {
    head: git(["rev-parse", "HEAD"]),
    dirtyFiles: dirty ? dirty.split("\n").filter(Boolean).length : null,
    snapshot: snapshotInputs(cwd),
  }
}

const TIMING_KEYS = [
  "wall_ms",
  "emit_ms",
  "emit_calls",
  "codec_ms",
  "codec_calls",
  "peephole_ms",
  "optimize_ms",
  "lex_calls",
  "closers_calls",
  "idle_fold_calls",
  "active_fold_calls",
]

// The compiler prints one `lilscript-timing {...}` line per compile under
// LILSCRIPT_TIMING=1. A port that emits several artifacts prints several; sum
// the counters and keep the max wall, which is what "did this port's build cost
// more" means.
function parseTiming(text) {
  const lines = text.split("\n").filter((l) => l.includes("lilscript-timing {"))
  if (!lines.length) return null
  const totals = { lines: lines.length }
  for (const line of lines) {
    const at = line.indexOf("{")
    let row
    try {
      row = JSON.parse(line.slice(at))
    } catch {
      continue
    }
    for (const key of TIMING_KEYS) {
      if (typeof row[key] !== "number") continue
      if (key === "wall_ms") totals[key] = Math.max(totals[key] ?? 0, row[key])
      else totals[key] = (totals[key] ?? 0) + row[key]
    }
  }
  return totals
}

// The heart of the "do not believe a build that did not happen" rule.
function classifyBuild(evidence) {
  const errors = validateBuild(evidence)
  return errors.length ? { trust: "untrustworthy", why: errors.join("; ") } : { trust: "ok", why: null }
}

// Failing-test SET, not a bit. `node --test` speaks TAP; jest prints ✕/●. Both
// are best-effort: the exit code is authoritative and the tail is always kept,
// so an unparsed runner degrades to "we know it failed" rather than to silence.
function extractFailures(text) {
  const failures = new Set()
  for (const line of text.split("\n")) {
    const tap = /^\s*not ok \d+\s*-?\s*(.+?)\s*$/.exec(line)
    if (tap) {
      failures.add(tap[1].trim())
      continue
    }
    const jest = /^\s*[✕×]\s+(.+?)(\s+\(\d+\s*ms\))?\s*$/.exec(line)
    if (jest) failures.add(jest[1].trim())
  }
  return [...failures].sort()
}

async function runPort(port, { compiler, pinnedCodec, timeoutsMs, skipTests, logDir, armDirectory }) {
  if (!/^[a-z][a-z0-9-]*$/.test(port)) throw new Error(`invalid workload id: ${port}`)
  const original = sourceWorkspace(port)
  const workload = workloadFor(port)
  if (!existsSync(join(original, "scripts/build.mjs"))) return { port, source: null, artifacts: [], tests: null, build: { trust: "untrustworthy", why: "required workload needs a build adapter or recovered workspace" } }
  const source = portSource(original)
  const cwd = join(armDirectory, "workspaces", port)
  cpSync(original, cwd, { recursive: true, filter: (path) => {
    const part = relative(original, path)
    return !part.split("/").some((name) => name === ".git" || name === "node_modules") && part !== "dist" && !part.startsWith("dist/")
  } })
  if (existsSync(join(original, "node_modules"))) symlinkSync(join(original, "node_modules"), join(cwd, "node_modules"), "dir")
  const nodeAdapter = workload?.tests?.nodeAdapter
  const vitestAdapter = workload?.tests?.vitestAdapter
  if (nodeAdapter?.sharedFixture) {
    const fixtureDirectory = join(cwd, ".lilscript-test-adapter")
    mkdirSync(fixtureDirectory, { recursive: true })
    writeFileSync(join(fixtureDirectory, "suite.test.mjs"), readFileSync(join(repo, "finer/tools/port-adapters", nodeAdapter.sharedFixture)))
  }
  const receiptDirectory = join(armDirectory, "invocations", port)
  const wrapperDirectory = join(armDirectory, "wrappers")
  mkdirSync(wrapperDirectory, { recursive: true })
  const compilerSha256 = sha256(compiler)
  const contractSha256 = fingerprint({ source: source.snapshot.sha256, build: "scripts/build.mjs --compile", outputRoots: ["dist"] })
  const settingsPath = join(wrapperDirectory, `${port}.json`)
  writeFileSync(settingsPath, JSON.stringify({ compiler, compilerSha256, contractSha256, receiptDirectory, outputRoots: ["dist"] }))
  const wrapper = join(wrapperDirectory, `${port}.mjs`)
  writeFileSync(wrapper, `#!${process.execPath}\nimport { runCompilerReceipt } from ${JSON.stringify(pathToFileURL(join(repo, "finer/tools/compiler-receipt.mjs")).href)};\ntry { process.exitCode = runCompilerReceipt(${JSON.stringify(settingsPath)}, process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }\n`, { mode: 0o755 })
  const env = {
    ...process.env,
    LILSCRIPT_COMPILER: wrapper,
    LILSCRIPT_ROOT: repo,
    LILSCRIPT_TIMING: "1",
  }

  // katexlil (and any future port with an mtime cache) skips compilation
  // without this; every other build script ignores an unknown flag or accepts
  // it, so probing the script text is cheaper than a per-port table.
  const script = readFileSync(join(cwd, "scripts/build.mjs"), "utf8")
  const buildArgs = ["scripts/build.mjs", "--compile"]
  if (script.includes('"--force"') || script.includes("'--force'")) buildArgs.push("--force")

  log(`${port}: building (${buildArgs.slice(1).join(" ")})`)
  const started = Date.now()
  const build = await runBoundedCommand(process.execPath, buildArgs, {
    cwd,
    env,
    encoding: "utf8",
    timeoutMs: timeoutsMs.build,
    maxBuffer: 1 << 28,
  })
  const elapsedMs = Date.now() - started
  const buildText = `${build.stdout ?? ""}\n${build.stderr ?? ""}`
  writeFileSync(join(logDir, `${port}.build.log`), buildText)

  const timing = parseTiming(buildText)
  const invocations = existsSync(receiptDirectory) ? readdirSync(receiptDirectory).filter((name) => name.endsWith(".json")).sort().map((name) => JSON.parse(readFileSync(join(receiptDirectory, name), "utf8"))) : []
  const verdict = classifyBuild({ exitCode: build.status, invocations, compilerSha256, contractSha256 })

  const record = {
    port,
    source,
    workspace: cwd,
    contractSha256,
    build: {
      args: buildArgs,
      status: build.status,
      signal: build.signal,
      error: build.error?.message ?? null,
      supervision: build.supervision,
      elapsedMs,
      trust: verdict.trust,
      why: verdict.why,
      timing,
      invocations,
      tail: buildText.trim().split("\n").slice(-12).join("\n"),
    },
    artifacts: [],
    tests: null,
  }

  if (verdict.trust === "ok") {
    const paths = portArtifacts(cwd)
    if (!paths.length) {
      record.build.trust = "untrustworthy"
      record.build.why = "build produced no JavaScript artifacts"
      return record
    }
    const measurement = await measure(paths, pinnedCodec, timeoutsMs.codec)
    record.measurement = measurement.evidence
    if (measurement.error) {
      record.build.trust = "untrustworthy"
      record.build.why = measurement.error
      return record
    }
    const sizes = measurement.sizes
    record.artifacts = paths.map((path) => {
      const row = sizes[resolve(path)]
      return {
        path: relative(cwd, path),
        objective: workload?.comparisons?.find((unit) => unit.artifact === relative(cwd, path))?.objective ?? null,
        sha256: sha256(path),
        raw: row.raw,
        gzip9: row.gzip9,
        brotli11: row.brotli11,
      }
    })
  }

  if (!skipTests && verdict.trust === "ok") {
    log(`${port}: testing`)
    if (nodeAdapter || vitestAdapter) {
      if (nodeAdapter && vitestAdapter) throw new Error(`ambiguous production test adapter: ${port}`)
      const adapter = nodeAdapter ?? vitestAdapter
      const inventory = adapter.caseInventory ? JSON.parse(readFileSync(resolve(dirname(workloadPath), adapter.caseInventory), "utf8")) : null
      if (inventory?.workload !== port || inventory?.schemaVersion !== 1 || inventory?.kind !== (nodeAdapter ? "required-node-case-inventory" : "required-vitest-case-inventory")) throw new Error(`invalid production case inventory: ${port}`)
      const options = { cwd, artifactPaths: adapter.artifacts, requiredCases: inventory.requiredCases, requiredTestFiles: inventory.testFiles, requiredFixtures: inventory.fixtures, directory: join(armDirectory, "tests", port), timeoutMs: timeoutsMs.test }
      const report = nodeAdapter
        ? await runNodeTestEvidence({ ...options, files: [...(adapter.files ?? []), ...(adapter.sharedFixture ? [".lilscript-test-adapter/suite.test.mjs"] : [])], requiredFilePatterns: inventory.entryPatterns, prerequisites: inventory.prerequisites === undefined ? [] : inventory.prerequisites, nodeArguments: adapter.nodeArguments ?? [] })
        : await runVitestTestEvidence({ ...options, config: adapter.config, requiredConfig: inventory.configuration, forbiddenSources: adapter.forbiddenSources ?? [] })
      const errors = [...report.errors, ...(inventory.additionalRequiredCoverage ?? []).map(coverage => `additional required coverage remains unverified: ${typeof coverage === "string" ? coverage : JSON.stringify(coverage)}`)]
      record.tests = { status: report.evidence.exitCode, certification: errors.length ? "unverified" : report.certification, evidence: report.evidence, errors, report: join(armDirectory, "tests", port, "report.json"), failing: report.evidence.cases.filter((row) => row.status !== "pass").map((row) => row.id) }
      return record
    }
    const test = await runBoundedCommand("npm", ["test", "--silent"], {
      cwd,
      env,
      encoding: "utf8",
      timeoutMs: timeoutsMs.test,
      maxBuffer: 1 << 28,
    })
    const testText = `${test.stdout ?? ""}\n${test.stderr ?? ""}`
    writeFileSync(join(logDir, `${port}.test.log`), testText)
    record.tests = {
      status: test.status,
      signal: test.signal,
      error: test.error?.message ?? null,
      supervision: test.supervision,
      certification: "unverified",
      certificationReason: "required case inventory and exact loaded production artifacts need a declared test adapter",
      failing: extractFailures(testText),
      tail: testText.trim().split("\n").slice(-20).join("\n"),
    }
    const altered = record.artifacts.filter((artifact) => !existsSync(join(cwd, artifact.path)) || sha256(join(cwd, artifact.path)) !== artifact.sha256)
    if (altered.length) {
      record.tests.certificationReason = `tests changed measured artifacts: ${altered.map((artifact) => artifact.path).join(", ")}`
    }
  }

  return record
}

async function record() {
  const milliseconds = (name, fallback) => {
    const value = Number(flag(name, fallback)) * 1000
    if (!Number.isSafeInteger(value) || value <= 0 || value > 2147483647) die(`--${name} must specify positive, finite, millisecond-compatible seconds within Node's timer range`)
    return value
  }
  const timeoutMs = milliseconds("timeout", 2700)
  const timeoutsMs = Object.fromEntries(["build", "test", "codec"].map(phase => [phase, milliseconds(`${phase}-timeout`, timeoutMs / 1000)]))
  const arm = flag("arm", "baseline")
  const compiler = resolve(flag("compiler", join(repo, "target/release/lilscript")))
  if (!existsSync(compiler)) die(`compiler ${compiler} does not exist`)

  const out = resolve(flag("out", join(repo, "finer/out/portgate", arm)))
  if (existsSync(out)) die(`arm destination already exists: ${out}; use a new --out to preserve its evidence`)
  const logDir = join(out, "logs")
  mkdirSync(logDir, { recursive: true })

  const only = (flag("ports", "") || "").split(",").filter(Boolean)
  const discovered = discoverPorts()
  const missing = only.filter((port) => !discovered.includes(port))
  if (missing.length) die(`requested ports are unavailable to this build adapter: ${missing.join(", ")}`)
  const ports = discovered.filter((p) => !only.length || only.includes(p))
  if (!ports.length) die("no ports discovered")

  const skipTests = has("skip-tests")

  // Copy the binary into the arm directory and record its digest. Two arms must
  // never reference target/release/lilscript, because a concurrent session
  // rebuilding it mid-run silently makes the comparison meaningless.
  const armCompiler = join(out, "lilscript")
  writeFileSync(armCompiler, readFileSync(compiler))
  execFileSync("chmod", ["+x", armCompiler])
  const armCodec = join(out, "lilscript-codec")
  const selectedCodec = resolve(flag("codec", codec))
  writeFileSync(armCodec, readFileSync(selectedCodec))
  execFileSync("chmod", ["+x", armCodec])

  const manifest = {
    arm,
    timeoutsMs,
    compiler: { source: compiler, sha256: sha256(armCompiler) },
    codec: { source: selectedCodec, ...fileIdentity(armCodec) },
    workloadManifest: {
      path: workloadPath,
      sha256: existsSync(workloadPath) ? sha256(workloadPath) : null,
      testContracts: [...new Set(workloadEntries.map((row) => (row.tests?.nodeAdapter ?? row.tests?.vitestAdapter)?.caseInventory).filter(Boolean))].sort().map((path) => ({ path, ...fileIdentity(resolve(dirname(workloadPath), path)) })),
    },
    repoHead: spawnSync("git", ["rev-parse", "HEAD"], { cwd: repo, encoding: "utf8" }).stdout?.trim(),
    host: { cpus: (process.report?.getReport?.()?.header?.cpus ?? []).length || null },
    ports: [],
  }
  log(`arm ${arm}: compiler ${manifest.compiler.sha256.slice(0, 12)}, ${ports.length} ports`)

  for (const port of ports) {
    const row = await runPort(port, { compiler: armCompiler, pinnedCodec: armCodec, timeoutsMs, skipTests, logDir, armDirectory: out })
    writeFileSync(join(out, `${port}.json`), `${JSON.stringify(row, null, 2)}\n`)
    manifest.ports.push(port)
    const t = row.build.trust
    const failing = row.tests?.failing?.length
    log(
      `${port}: build=${t}${t === "ok" ? ` ${(row.build.elapsedMs / 1000).toFixed(0)}s` : ` (${row.build.why})`}` +
        (row.tests ? ` tests=${row.tests.status === 0 ? "pass" : `${failing} failing`} evidence=${row.tests.certification}` : "")
    )
  }

  writeFileSync(join(out, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`)

  const rows = manifest.ports.map((p) => JSON.parse(readFileSync(join(out, `${p}.json`), "utf8")))
  const untrusted = rows.filter((r) => r.build.trust !== "ok")
  log(`recorded ${rows.length} ports to ${out}`)
  if (untrusted.length) {
    log(`UNTRUSTWORTHY BUILDS (${untrusted.length}): ${untrusted.map((r) => r.port).join(", ")}`)
    log("a build that did not happen is a failed run, not a fast one")
    process.exitCode = 1
    return
  }
  const unverifiedTests = skipTests ? [] : rows.filter((row) => row.tests?.certification !== "verified")
  if (unverifiedTests.length) {
    log(`UNVERIFIED REQUIRED TESTS (${unverifiedTests.length}): ${unverifiedTests.map((row) => row.port).join(", ")}`)
    log("evidence is retained; an inventory run or failing suite cannot return a verified gate result")
    process.exitCode = 1
    return
  }
  if (skipTests) log("diagnostic build/measurement only: production tests were explicitly skipped")
}

function compare() {
  const [aDir, bDir] = argv.slice(1).filter((a) => !a.startsWith("--")).map((d) => resolve(d))
  if (!aDir || !bDir) die("usage: portgate.mjs compare <baselineDir> <candidateDir>")

  const readArm = (dir) => {
    const manifest = JSON.parse(readFileSync(join(dir, "manifest.json"), "utf8"))
    const ports = Object.fromEntries(
      manifest.ports.map((p) => [p, JSON.parse(readFileSync(join(dir, `${p}.json`), "utf8"))])
    )
    return { manifest, ports }
  }
  const a = readArm(aDir)
  const b = readArm(bDir)
  if (!a.manifest.ports.length || !b.manifest.ports.length) die("an empty arm cannot pass comparison")
  if (fingerprint([...a.manifest.ports].sort()) !== fingerprint([...b.manifest.ports].sort())) die("arms must cover the same required workload set")
  if (!a.manifest.workloadManifest?.sha256 || a.manifest.workloadManifest.sha256 !== b.manifest.workloadManifest?.sha256) die("arms must use the same workload and test contracts")
  if (fingerprint(a.manifest.workloadManifest.testContracts) !== fingerprint(b.manifest.workloadManifest.testContracts)) die("required test contract contents differ between arms")
  if (!a.manifest.codec?.sha256 || a.manifest.codec.sha256 !== b.manifest.codec?.sha256) die("arms must name the same pinned codec binary")

  if (a.manifest.compiler.sha256 === b.manifest.compiler.sha256) {
    log("WARNING: both arms used the same compiler digest -- this compares nothing")
  }

  let regressions = 0
  const lines = []
  for (const port of Object.keys(a.ports)) {
    const left = a.ports[port]
    const right = b.ports[port]
    if (!right) {
      lines.push(`${port}: MISSING from candidate arm`)
      regressions += 1
      continue
    }
    if (left.build.trust !== "ok" || right.build.trust !== "ok") {
      lines.push(`${port}: build not trustworthy (${left.build.trust} -> ${right.build.trust})`)
      regressions += 1
      continue
    }
    if (!left.source?.snapshot?.sha256 || left.source.snapshot.sha256 !== right.source?.snapshot?.sha256) {
      lines.push(`${port}: source/configuration contents differ or their identity is missing`)
      regressions += 1
      continue
    }
    const invocationInputs = (row) => row.build.invocations.map((invocation) => ({
      inputSha256: invocation.inputSha256,
      args: invocation.args.map((arg) => arg.replaceAll(invocation.cwd, "$WORKSPACE")),
      output: relative(invocation.cwd, invocation.output.path),
    })).sort((x, y) => JSON.stringify(x).localeCompare(JSON.stringify(y)))
    if (fingerprint(invocationInputs(left)) !== fingerprint(invocationInputs(right))) {
      lines.push(`${port}: actual compiler inputs or invocation profiles differ`)
      regressions += 1
      continue
    }
    for (const [label, row, manifest] of [["baseline", left, a.manifest], ["candidate", right, b.manifest]]) {
      const buildErrors = validateBuild({ exitCode: row.build.status, invocations: row.build.invocations, compilerSha256: manifest.compiler.sha256, contractSha256: row.contractSha256 })
      const testErrors = row.tests?.evidence ? validateTests(row.tests.evidence) : ["production test-case and loaded-artifact evidence is missing"]
      if (row.tests?.certification !== "verified") testErrors.push("production adapter evidence is not verified")
      if (buildErrors.length || testErrors.length) {
        lines.push(`${port}/${label}: ${[...buildErrors, ...testErrors].join("; ")}`)
        regressions += 1
      }
    }
    if (!left.artifacts?.length || !right.artifacts?.length) {
      lines.push(`${port}: required artifact inventory is empty`)
      regressions += 1
      continue
    }

    // Every exact byte matters. Identity is stricter than equal codec size and
    // is available for phases claiming output-neutral plumbing.
    const selectedArtifacts = left.artifacts.filter((artifact) => ["raw", "gzip", "brotli"].includes(artifact.objective))
    if (!selectedArtifacts.length) {
      lines.push(`${port}: no declared objective-specific comparison artifacts`)
      regressions += 1
    }
    const rightByPath = Object.fromEntries(right.artifacts.map((x) => [x.path, x]))
    for (const artifact of selectedArtifacts) {
      const other = rightByPath[artifact.path]
      if (!other) {
        lines.push(`${port}/${artifact.path}: artifact disappeared`)
        regressions += 1
        continue
      }
      if (artifact.objective !== other.objective) {
        lines.push(`${port}/${artifact.path}: objective changed between arms`)
        regressions += 1
        continue
      }
      for (const [label, row, selected] of [["baseline", left, artifact], ["candidate", right, other]]) {
        if (!row.tests?.evidence?.requiredArtifacts?.some((tested) => resolve(tested.path) === resolve(row.workspace, selected.path) && tested.sha256 === selected.sha256)) {
          lines.push(`${port}/${label}/${selected.path}: selected artifact is absent from required production tests`)
          regressions += 1
        }
      }
      if (![artifact, other].every((row) => ["raw", "gzip9", "brotli11"].every((key) => Number.isSafeInteger(row[key]) && row[key] >= 0))) {
        lines.push(`${port}/${artifact.path}: incomplete codec measurement`)
        regressions += 1
        continue
      }
      if (other.sha256 === artifact.sha256) continue
      const metric = { raw: "raw", gzip: "gzip9", brotli: "brotli11" }[artifact.objective]
      const delta = other[metric] - artifact[metric]
      const sign = delta > 0 ? "+" : ""
      lines.push(
        `${port}/${artifact.path}: bytes changed ${sign}${delta} ${artifact.objective} (${artifact[metric]} -> ${other[metric]})`
      )
      if (delta > 0 || has("require-identical")) regressions += 1
    }

    // Failure-set changes remain useful diagnosis. The full production test
    // evidence above is mandatory even when this historical set is unchanged.
    const before = new Set(left.tests?.failing ?? [])
    const after = new Set(right.tests?.failing ?? [])
    const appeared = [...after].filter((t) => !before.has(t))
    const fixed = [...before].filter((t) => !after.has(t))
    if (appeared.length) {
      lines.push(`${port}: ${appeared.length} NEW failing tests: ${appeared.slice(0, 5).join(" | ")}`)
      regressions += 1
    }
    if (fixed.length) lines.push(`${port}: ${fixed.length} tests newly passing`)
    if ((left.tests?.status ?? 0) === 0 && (right.tests?.status ?? 0) !== 0 && !appeared.length) {
      lines.push(`${port}: suite exit ${left.tests.status} -> ${right.tests.status} with no parsed failure names`)
      regressions += 1
    }

    // Cost, per D9: record compile time with size.
    const lt = left.build.timing ?? {}
    const rt = right.build.timing ?? {}
    if (lt.emit_ms && rt.emit_ms) {
      const ratio = rt.emit_ms / lt.emit_ms
      if (ratio > 1.05 || ratio < 0.95) {
        lines.push(`${port}: emit_ms ${Math.round(lt.emit_ms)} -> ${Math.round(rt.emit_ms)} (${ratio.toFixed(2)}x)`)
      }
    }
  }

  process.stdout.write(`${lines.join("\n")}\n`)
  process.stdout.write(
    regressions
      ? `\nFAIL: ${regressions} regression(s) across ${Object.keys(a.ports).length} ports\n`
      : `\nOK: no regressions across ${Object.keys(a.ports).length} ports\n`
  )
  process.exitCode = regressions ? 1 : 0
}

try {
  switch (command) {
    case "record":
      await record()
      break
    case "compare":
      compare()
      break
    default:
      die("usage: portgate.mjs record --arm <name> --compiler <path> | portgate.mjs compare <dirA> <dirB>")
  }
} catch (error) {
  log(error.message)
  process.exitCode = 1
}
