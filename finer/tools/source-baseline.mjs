// Build one library from source with a pinned compiler, measure what it
// delivers, and run its frozen required cases against those exact bytes.
//
// 001: "Build valid incumbents from source; tests execute the scored artifacts.
// A failed build cannot inherit stale `dist`; a successful package command alone
// does not prove which bytes ran."
//
// So this driver does three things in one receipt, and refuses to separate them:
//
//   1. compiles the port's own sources with a named compiler binary, in a
//      scratch copy, under the per-profile wall and memory envelope
//   2. measures every produced artifact with the repository's canonical codec
//   3. runs the port's frozen required-case inventory against the built
//      artifacts and records which artifact each case actually loaded
//
// A run that fails at any step is preserved with its reason. It never falls
// back to the committed `dist`, and it never reports sizes for bytes no test
// executed.

import assert from "node:assert/strict"
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs"
import { basename, dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity, fingerprint, snapshotInputs, snapshotDependencyTree } from "./artifact-evidence.mjs"
import { runBoundedCommand } from "./bounded-command.mjs"
import { runNodeTestEvidence } from "./node-test-evidence.mjs"
import { measure } from "./competitor-recipes.mjs"
import { ENVELOPES } from "./cost-policy.mjs"
import { preserveBinary } from "./preserved-binaries.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
const root = resolve(toolsDirectory, "../..")
const WORKSPACE_EXCLUDE = [".git", "node_modules", "target", ".cache"]
const identify = path => ({ path, ...fileIdentity(path) })

/**
 * Environment variables a port's build script reads to find the compiler.
 *
 * Ports do not agree on one name: `solidlil` reads `SOLIDLIL_LILSCRIPT_BIN`,
 * most read `LILSCRIPT_COMPILER`. Rather than keep a table that drifts, read
 * the build script and set every compiler-shaped variable it actually names, so
 * the pinned binary is what runs and the receipt records which names were used.
 */
export function compilerVariables(buildScriptPath) {
  const names = new Set(["LILSCRIPT_COMPILER"])
  if (existsSync(buildScriptPath)) {
    const source = readFileSync(buildScriptPath, "utf8")
    for (const match of source.matchAll(/process\.env\.([A-Z0-9_]+)/g)) {
      if (/LILSCRIPT|COMPILER/.test(match[1]) && !/CONFIG|LEVEL|SKIP|ONLY|SNIPPET/.test(match[1])) names.add(match[1])
    }
  }
  return [...names].sort()
}

/** Directory symlinks, which the input snapshot requires to be their own root. */
export function directorySymlinks(workspace, exclude) {
  const found = []
  const walk = (directory, prefix) => {
    for (const entry of readdirSync(directory, { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name))) {
      const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name
      if (exclude.includes(relativePath.split("/")[0])) continue
      const absolute = join(directory, entry.name)
      if (entry.isSymbolicLink()) {
        try { if (!statSync(absolute).isFile()) found.push(relativePath) } catch { found.push(relativePath) }
      } else if (entry.isDirectory()) walk(absolute, relativePath)
    }
  }
  walk(workspace, "")
  return found
}

/**
 * Sibling checkouts a port's sources import by relative path.
 *
 * Several ports compile a sibling port's sources directly, e.g.
 * `import { toHtml } from "../../../hast-util-to-htmllil/src/index.lil"`. An
 * isolated copy has no siblings, so the build must be given the same layout;
 * this finds which top-level directories next to the workspace are reached.
 */
export function referencedSiblings(workspace) {
  const found = new Set()
  const parent = dirname(workspace)
  const visit = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const absolute = join(directory, entry.name)
      if (entry.isDirectory() && !["node_modules", ".git", "dist", "target"].includes(entry.name)) visit(absolute)
      else if (entry.isFile() && entry.name.endsWith(".lil")) {
        for (const match of readFileSync(absolute, "utf8").matchAll(/from\s+"(\.\.\/[^"]+)"/g)) {
          const target = resolve(directory, match[1])
          const outside = relative(parent, target)
          const top = outside.split("/")[0]
          if (!outside.startsWith("..") && top && top !== basename(workspace)) found.add(top)
        }
      }
    }
  }
  if (existsSync(join(workspace, "src"))) visit(join(workspace, "src"))
  return [...found].sort()
}

/** The first recorded workspace candidate that is actually usable. */
export function resolveWorkspace(library) {
  const candidates = [library.workspace, ...(library.workspaceCandidates ?? [])].filter(Boolean)
  const usable = candidates.find(path => existsSync(join(path, "package.json")) && existsSync(join(path, "node_modules")))
  return { chosen: usable ?? library.workspace, candidates, recorded: library.workspace, substituted: Boolean(usable) && usable !== library.workspace }
}

/** Every JavaScript file a build produced under `dist`. */
export function producedArtifacts(workspace) {
  const directory = join(workspace, "dist")
  if (!existsSync(directory)) return []
  return readdirSync(directory).filter(name => /\.(?:js|cjs|mjs)$/.test(name)).sort().map(name => `dist/${name}`)
}

/**
 * Build and qualify one library.
 *
 * `library` is a workload-manifest row. `inventory` is its frozen
 * required-case inventory, or null when it has none yet — in which case the
 * run measures bytes and says plainly that nothing qualified them.
 */
export async function buildSourceBaseline({ library, inventory, directory, compiler = join(root, "target/release/lilscript"), codec = join(root, "target/release/lilscript-codec"), node = process.execPath, wallSeconds = ENVELOPES.compileWallSecondsPerProfile.ceiling }) {
  const started = performance.now()
  // Run the build with an immutable copy, so a later `cargo build` cannot
  // change the binary this receipt names.
  const preservedCompiler = preserveBinary(compiler)
  const preservedCodec = preserveBinary(codec)
  compiler = preservedCompiler.path
  codec = preservedCodec.path
  const workspaceChoice = resolveWorkspace(library)
  const original = workspaceChoice.chosen
  const npm = join(dirname(node), "npm")
  const scratch = mkdtempSync(`/tmp/lilscript-baseline-${library.id}-`)
  const workspace = join(scratch, basename(original))
  mkdirSync(directory, { recursive: true })
  const save = (name, value) => writeFileSync(join(directory, name), JSON.stringify(value, null, 2) + "\n")

  const receipt = {
    schema: 1, kind: "source-built-library-baseline", library: library.id,
    started: new Date().toISOString(), passed: false, sourceBuilt: false, qualification: "unverified",
    original, workspace, workspaceChoice, compiler: preservedCompiler, codec: preservedCodec, node: identify(node),
    envelope: { wallSeconds, residentMegabytes: ENVELOPES.compilerResidentMegabytes.ceiling },
    limitations: [
      "One build of one library with one compiler binary. This is not a fleet result and not a competitor comparison.",
      "Sizes are the delivered artifacts this build produced, measured by the repository's canonical codec.",
      "Case coverage is exactly the frozen required inventory; coverage outside it remains unverified.",
    ],
  }
  let before
  save("receipt.json", receipt)
  try {
    assert(existsSync(compiler), `compiler binary not found: ${compiler}`)
    const buildCommand = library.build?.recordedCommand
    assert(buildCommand, "the workload manifest records no build command for this library")
    receipt.buildCommand = buildCommand
    const tokens = buildCommand.split(/\s+/)
    assert.equal(tokens[0], "node", `only a node build command is supported here: ${buildCommand}`)

    cpSync(original, workspace, { recursive: true, verbatimSymlinks: true, filter: path => !WORKSPACE_EXCLUDE.includes(relative(original, path).split("/")[0]) })
    if (existsSync(join(original, "node_modules"))) symlinkSync(join(original, "node_modules"), join(workspace, "node_modules"), "dir")
    // Give the copy the same neighbours the original has for the siblings its
    // sources import, and pin those siblings' sources as inputs of this build.
    receipt.siblingSources = []
    for (const sibling of referencedSiblings(original)) {
      const home = join(dirname(original), sibling)
      if (!existsSync(home)) { receipt.siblingSources.push({ name: sibling, missing: true }); continue }
      symlinkSync(home, join(dirname(workspace), sibling), "dir")
      const snapshot = snapshotInputs(home, { paths: existsSync(join(home, "src")) ? ["src"] : ["."] })
      receipt.siblingSources.push({ name: sibling, root: home, files: snapshot.files.length, sha256: snapshot.sha256 })
    }

    // A directory symlink cannot be hashed in place; name the ones skipped
    // rather than letting the snapshot fail or quietly cover less.
    const skipped = directorySymlinks(original, WORKSPACE_EXCLUDE)
    receipt.unsnapshottedDirectorySymlinks = skipped
    if (skipped.length) receipt.limitations.push(`These directory symlinks are outside the input snapshot and their contents are unpinned: ${skipped.join(", ")}.`)
    const snapshotExclude = [...WORKSPACE_EXCLUDE, ...skipped]
    const full = target => snapshotInputs(target, { exclude: snapshotExclude })
    const capture = () => ({
      source: snapshotInputs(original, { exclude: [".git", "node_modules", "dist", "target", ".cache", ...skipped] }), original: full(original),
      project: snapshotDependencyTree(join(original, "node_modules"), { maxBytes: 8 * 1024 * 1024 * 1024, maxEntries: 400_000 }),
    })
    before = capture()
    save("before.json", before)
    receipt.sourceSnapshot = { root: original, files: before.source.files.length, sha256: before.source.sha256 }
    // The compiler, the runtime and this boundary's configurations are what a
    // later reader must be able to re-check; pin them by identity.
    receipt.inputs = {
      files: [identify(compiler), identify(codec), identify(node),
        ...(library.configurations ?? []).map(configuration => join(original, configuration.path)).filter(existsSync).map(identify)],
    }

    // The committed distribution must not be able to stand in for a build that
    // did not happen, so it is removed from the scratch copy first. Removed, not
    // emptied: a stale file the build never writes (an old output name, a
    // side artifact) then simply does not exist afterwards, instead of being
    // mistaken for a build output that came out empty.
    const inherited = producedArtifacts(workspace)
    receipt.committedArtifactsRemoved = inherited
    for (const path of inherited) rmSync(join(workspace, path))

    const environment = { ...process.env, PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin` }
    for (const key of Object.keys(environment)) if (/^npm_/i.test(key) || key === "NODE_TEST_CONTEXT") delete environment[key]
    environment.NODE_OPTIONS = ""
    receipt.compilerVariables = compilerVariables(join(original, tokens[1] ?? "scripts/build.mjs"))
    for (const name of receipt.compilerVariables) environment[name] = compiler
    const buildStarted = performance.now()
    const build = await runBoundedCommand(node, tokens.slice(1), { cwd: workspace, env: environment, encoding: "utf8", timeoutMs: wallSeconds * 1000, maxBuffer: 1 << 28 })
    receipt.build = {
      status: build.status, signal: build.signal, error: build.error?.message ?? null,
      wallSeconds: Number(((performance.now() - buildStarted) / 1000).toFixed(3)), supervision: build.supervision,
    }
    writeFileSync(join(directory, "build.stdout"), build.stdout ?? "")
    writeFileSync(join(directory, "build.stderr"), build.stderr ?? "")
    if (build.status !== 0 || build.error || build.signal) {
      receipt.failure = { message: `build did not complete: status=${build.status} signal=${build.signal} ${build.error?.message ?? ""}`.trim() }
      receipt.limitations.push("The build did not complete, so no artifact is eligible and the committed distribution is not a substitute.")
      return finish()
    }
    if (receipt.build.wallSeconds > wallSeconds) {
      receipt.envelopeExceeded = `build wall ${receipt.build.wallSeconds}s exceeds the ${wallSeconds}s per-profile ceiling`
    }
    receipt.sourceBuilt = true

    const artifacts = producedArtifacts(workspace)
    assert(artifacts.length, "the build produced no JavaScript artifact")
    receipt.notRebuilt = inherited.filter(path => !artifacts.includes(path))
    if (receipt.notRebuilt.length) receipt.limitations.push(`The committed distribution also carried ${receipt.notRebuilt.join(", ")}, which this build does not produce; they are stale and are not measured.`)
    receipt.artifacts = artifacts.map(path => ({ path, ...fileIdentity(join(workspace, path)), sizes: measure(join(workspace, path), codec) }))
    receipt.measurements = Object.fromEntries(receipt.artifacts.map(row => [row.path, row.sizes]))
    for (const row of receipt.artifacts) copyFileSync(join(workspace, row.path), join(directory, basename(row.path)))

    if (!inventory) {
      receipt.limitations.push("This library has no frozen required-case inventory, so these bytes are measured but not qualified by any test.")
      return finish()
    }
    if (inventory.kind !== "required-node-case-inventory" || !inventory.entryPatterns?.length) {
      // A Vitest or profile-replay inventory has its own runner. Feeding it to
      // `node --test` would report every case as missing, which is a harness
      // failure dressed up as a compiler one.
      receipt.inventoryKind = inventory.kind
      receipt.limitations.push(`The frozen inventory is \`${inventory.kind}\`${inventory.entryPatterns?.length ? "" : " without entry patterns"}, which this driver does not execute; these bytes are measured but not qualified here.`)
      return finish()
    }
    // Qualification runs the frozen cases against the bytes just built.
    const requiredArtifacts = inventory.deliveryScope?.requiredArtifacts ?? inventory.nodeAdapter?.artifacts ?? artifacts
    const present = requiredArtifacts.filter(path => existsSync(join(workspace, path)))
    assert(present.length, `none of the inventory's required artifacts were produced: ${requiredArtifacts.join(", ")}`)
    const prerequisites = (inventory.prerequisites ?? []).map(prerequisite => ({
      ...prerequisite, executable: identify(npm),
      inputs: prerequisite.inputs.map(input => ({ path: input.path, ...fileIdentity(join(workspace, input.path)) })),
    }))
    const rewritten = new Set(inventory.mutatedFixtures ?? [])
    receipt.observedNotPinned = [...rewritten]
    const fixtures = (inventory.fixtures ?? []).filter(row => !rewritten.has(row.path) && existsSync(join(workspace, row.path)))
      .map(row => ({ path: row.path, ...fileIdentity(join(workspace, row.path)) }))
    // The compile envelope bounds compilation. Executing the frozen cases is
    // test time, which the plan asks to be logged separately, so it gets its
    // own budget rather than eating whatever the build left over.
    const testBudgetMs = Number(process.env.LILSCRIPT_BASELINE_TEST_MS ?? 900_000)
    // Some suites compile a fixture inside the test itself (rehypelil's
    // parse5-modules test does). They must reach the same pinned binary the
    // build used, not a sibling path the isolated copy does not have.
    for (const name of receipt.compilerVariables ?? ["LILSCRIPT_COMPILER"]) process.env[name] = compiler
    const report = await runNodeTestEvidence({
      cwd: workspace, files: inventory.testFiles.map(row => row.path), requiredFilePatterns: inventory.entryPatterns,
      nodeArguments: inventory.nodeArguments ?? [], artifactPaths: present, requiredCases: inventory.requiredCases,
      requiredTestFiles: null, requiredFixtures: fixtures, prerequisites, directory: join(directory, "node"), timeoutMs: testBudgetMs,
    })
    receipt.testWallSeconds = Number(((performance.now() - started) / 1000 - receipt.build.wallSeconds).toFixed(3))
    receipt.cases = { required: inventory.requiredCases.length, executed: report.evidence.cases.length, failed: report.evidence.cases.filter(row => row.status !== "pass").map(row => row.id) }
    receipt.testErrors = report.errors
    receipt.loadedArtifacts = report.evidence.loadedArtifacts.map(row => ({ path: relative(workspace, row.path), sha256: row.sha256, bytes: row.bytes }))
    receipt.requiredCases = report.evidence.cases.map(row => ({ id: row.id, status: row.status }))
    receipt.passed = report.errors.length === 0 && !receipt.envelopeExceeded
    receipt.qualification = receipt.passed ? "source-built-and-case-qualified" : "unverified"
  } catch (error) {
    receipt.failure = { message: error.message, stack: error.stack }
  }
  return finish()

  function finish() {
    try {
      if (before) {
        const skipped = receipt.unsnapshottedDirectorySymlinks ?? []
        const after = {
          source: snapshotInputs(original, { exclude: [".git", "node_modules", "dist", "target", ".cache", ...skipped] }),
          original: snapshotInputs(original, { exclude: [...WORKSPACE_EXCLUDE, ...skipped] }),
        }
        assert.deepEqual(after.source.sha256, before.source.sha256, "the original workspace changed during the build")
        assert.deepEqual(after.original.sha256, before.original.sha256, "the original workspace changed during the build")
        receipt.inputsStable = true
      }
    } catch (error) {
      receipt.passed = false
      receipt.inputsStable = false
      receipt.validationFailure = { message: error.message }
    }
    receipt.elapsedSeconds = Number(((performance.now() - started) / 1000).toFixed(3))
    receipt.completed = new Date().toISOString()
    receipt.fingerprint = fingerprint({ ...receipt, started: null, completed: null, elapsedSeconds: null })
    save("receipt.json", receipt)
    return receipt
  }
}

function readInventory(library) {
  const name = library.tests?.requiredCaseInventory
  if (!name) return null
  const path = join(root, "benchmarks/libraries", name)
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  const VALUE_FLAGS = new Set(["--run-id", "--compiler", "--wall-seconds"])
  const flags = new Map(), explicit = []
  for (let at = 0; at < argv.length; at += 1) {
    if (VALUE_FLAGS.has(argv[at])) { flags.set(argv[at], argv[at + 1]); at += 1 }
    else if (argv[at].startsWith("--")) flags.set(argv[at], true)
    else explicit.push(argv[at])
  }
  const manifest = JSON.parse(readFileSync(join(root, "benchmarks/libraries/maintained-workloads.json"), "utf8"))
  const rows = [...manifest.libraries, ...(manifest.additionalCoverage ?? [])]
  const selected = explicit.length
    ? explicit.map(id => rows.find(row => row.id === id)).filter(Boolean)
    : rows.filter(row => row.tests?.requiredCaseInventory)
  if (!selected.length) { console.error("no libraries selected"); process.exit(2) }
  const runId = flags.get("--run-id") ?? `${new Date().toISOString().slice(0, 10)}-source-baselines`
  const runDirectory = join(root, "benchmarks/migration-results", runId)
  const wallSeconds = Number(flags.get("--wall-seconds") ?? ENVELOPES.compileWallSecondsPerProfile.ceiling)
  const compiler = resolve(flags.get("--compiler") ?? join(root, "target/release/lilscript"))
  const summary = { schema: 1, kind: "source-baseline-batch", runId, compiler: preserveBinary(compiler), started: new Date().toISOString(), qualified: [], unqualified: [] }
  for (const library of selected) {
    process.stderr.write(`\n=== ${library.id} ===\n`)
    const receipt = await buildSourceBaseline({ library, inventory: readInventory(library), directory: join(runDirectory, library.id), compiler, wallSeconds })
    const row = {
      library: library.id, passed: receipt.passed, sourceBuilt: receipt.sourceBuilt,
      buildWallSeconds: receipt.build?.wallSeconds ?? null, artifacts: receipt.artifacts?.length ?? 0,
      cases: receipt.cases ?? null, measurements: receipt.measurements ?? null,
      envelopeExceeded: receipt.envelopeExceeded ?? null,
      reason: receipt.failure?.message ?? receipt.testErrors?.join("; ") ?? null,
    }
    ;(receipt.passed ? summary.qualified : summary.unqualified).push(row)
    process.stderr.write(`${receipt.passed ? "QUALIFIED" : "unqualified"} build=${row.buildWallSeconds}s artifacts=${row.artifacts} ${row.reason ? `reason=${row.reason.slice(0, 160)}` : ""}\n`)
  }
  summary.completed = new Date().toISOString()
  mkdirSync(runDirectory, { recursive: true })
  writeFileSync(join(runDirectory, "summary.json"), JSON.stringify(summary, null, 2) + "\n")
  console.log(JSON.stringify({ runId, qualified: summary.qualified.map(row => row.library), unqualified: summary.unqualified.map(row => `${row.library}: ${(row.reason ?? "").slice(0, 120)}`) }, null, 1))
}
