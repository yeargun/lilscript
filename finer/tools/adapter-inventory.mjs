// Freeze required-case inventories for maintained libraries, in batch.
//
// One `discoverAdapter` run per library, then the two artifacts 001 needs from
// it: `benchmarks/libraries/<id>.required-tests.json` and the `nodeAdapter`
// block on that library's row of the workload manifest. A library the discovery
// cannot account for is written into the summary with its reason and keeps its
// pending status, so the inventory stays complete with explicit gaps rather
// than quietly shrinking to the libraries that happened to work.

import { readFileSync, writeFileSync } from "node:fs"
import { dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity, fingerprint, snapshotInputs, snapshotDependencyTree } from "./artifact-evidence.mjs"
import { discoverAdapter } from "./adapter-discovery.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
const root = resolve(toolsDirectory, "../..")
const manifestPath = join(root, "benchmarks/libraries/maintained-workloads.json")

const readJson = path => JSON.parse(readFileSync(path, "utf8"))
const writeJson = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n")

/** Build the frozen case inventory for one passing discovery. */
export function buildInventory({ library, receipt, reportPath, workspace }) {
  const report = readJson(reportPath)
  const cases = report.evidence.cases
  const tests = cases.filter(row => row.type === "test")
  const suites = cases.filter(row => row.type === "suite")
  const source = snapshotInputs(library.workspace)
  const identity = path => ({ path, ...fileIdentity(join(library.workspace, path)) })
  const buildScript = source.files.some(row => row.path === "scripts/build.mjs") ? identity("scripts/build.mjs") : null
  const declared = library.package?.declaredEntryFiles ?? []
  return {
    schemaVersion: 1,
    kind: "required-node-case-inventory",
    workload: library.id,
    capturedFrom: { path: relative(root, reportPath), ...fileIdentity(reportPath) },
    review: `Derived from the library's own \`npm test\` script by finer/tools/adapter-derivation.mjs and observed once against the existing distribution. ${tests.length} tests and ${suites.length} suites executed and passed; ${receipt.exercisedArtifacts.length} of ${declared.length} declared entry files were loaded by the suite.`,
    originalCommand: receipt.originalTestScript,
    originalSegments: receipt.derivation.originalSegments,
    observedSegments: receipt.derivation.segments,
    droppedSegments: receipt.derivation.deviations,
    uncoveredRunners: receipt.derivation.uncovered,
    entryPatterns: receipt.derivation.patterns,
    nodeArguments: receipt.derivation.nodeArguments,
    requiredCases: cases.map(row => row.id).sort(),
    // Inputs the original suite rewrites while it runs. A qualification run
    // observes them instead of pinning them, or it would fail on the suite's
    // own behaviour.
    mutatedFixtures: receipt.mutatedFixtures ?? [],
    testFiles: report.testFiles.map(({ path, sha256, bytes }) => ({ path, sha256, bytes })),
    fixtures: readJson(join(dirname(reportPath), "../declarations.json")).fixtures,
    prerequisites: readJson(join(dirname(reportPath), "../declarations.json")).prerequisites,
    buildScript,
    sourceSnapshot: { root: library.workspace, files: source.files.length, sha256: source.sha256 },
    dependencySnapshot: snapshotDependencyTree(join(library.workspace, "node_modules")),
    observedInventory: {
      testCases: tests.length, suiteNodes: suites.length, totalIdentities: cases.length,
      skipped: cases.filter(row => row.status === "skip").length,
      failed: cases.filter(row => row.status !== "pass").length,
    },
    deliveryScope: {
      requiredArtifacts: receipt.exercisedArtifacts,
      declaredButUnexercised: receipt.untestedEntries,
      declaredButAbsent: receipt.artifacts.absent,
      sourceBuilt: false,
      package: "unverified: no pack, install or packaged-consumer observation",
      declarations: "unverified: type declarations are not exercised by this command",
      site: "as the original command covers it; no browser execution",
    },
    sourceBuildGaps: [
      "No compiler ran: this inventory pins the existing distribution, not a source-built incumbent.",
      ...receipt.derivation.deviations,
      ...(receipt.untestedEntries.length ? [`Declared entry files the original suite never loads: ${receipt.untestedEntries.join(", ")}. Their delivery remains unqualified.`] : []),
      ...(receipt.artifacts.absent.length ? [`Declared entry files absent from the workspace: ${receipt.artifacts.absent.join(", ")}.`] : []),
      ...(receipt.mutatedFixtures?.length ? [`The original suite rewrites these inputs while it runs: ${receipt.mutatedFixtures.join(", ")}. They are observed, not pinned.`] : []),
    ],
    additionalRequiredCoverage: [
      "CJS, UMD and browser boundaries beyond what the original command loads.",
      "Packaged install and downstream consumer observation.",
      "Type declaration checking where the original command does not perform it.",
      "Source-built qualification and codec measurement (001 baselines).",
    ],
  }
}

/** The `nodeAdapter` block the workload manifest carries for this library. */
export function manifestAdapter({ library, receipt, inventoryName }) {
  const declared = library.package?.declaredEntryFiles ?? []
  const scopeNotes = [
    `Original \`npm test\` command as declared, parsed into ${receipt.derivation.prerequisites.length} prerequisite script(s) and one node --test selection.`,
    `Existing distribution only; ${receipt.exercisedArtifacts.length} of ${declared.length} declared entry files exercised.`,
  ]
  if (receipt.derivation.deviations.length) scopeNotes.push(...receipt.derivation.deviations)
  if (receipt.untestedEntries.length) scopeNotes.push(`Unexercised declared entries: ${receipt.untestedEntries.join(", ")}.`)
  return {
    files: receipt.derivation.patterns,
    artifacts: receipt.exercisedArtifacts,
    caseInventory: inventoryName,
    derivedFrom: "package.json scripts.test",
    scope: scopeNotes.join(" "),
  }
}

/**
 * Re-observe libraries that already have a frozen inventory and check that the
 * original suite still produces exactly those case identities, all passing.
 *
 * Freezing an inventory once proves it existed; this proves it is still true.
 * A case that appears, disappears or stops passing is drift in the suite, the
 * distribution or the environment, and is reported rather than re-frozen.
 */
async function verifyFrozen({ manifest, explicit, flag }) {
  const withInventory = manifest.libraries.filter(library => library.tests?.nodeAdapter && library.tests?.requiredCaseInventory)
  const selected = explicit.length ? withInventory.filter(library => explicit.includes(library.id)) : withInventory
  const runId = flag("--run-id") ?? `${new Date().toISOString().slice(0, 10)}-inventory-verification`
  const runDirectory = join(root, "benchmarks/migration-results", runId)
  const timeoutMs = Number(flag("--timeout-ms") ?? 300_000)
  const results = []
  for (const library of selected) {
    const inventoryPath = join(root, "benchmarks/libraries", library.tests.requiredCaseInventory)
    const inventory = readJson(inventoryPath)
    if (inventory.kind !== "required-node-case-inventory" || !inventory.entryPatterns?.length) {
      results.push({ library: library.id, status: "not-verifiable-here", reason: `inventory kind ${inventory.kind}` })
      continue
    }
    process.stderr.write(`\n=== verify ${library.id} ===\n`)
    const directory = join(runDirectory, library.id)
    const receipt = await discoverAdapter({ library, directory, timeoutMs })
    const observed = (receipt.observedCases ?? []).map(row => row.id).sort()
    const frozen = [...inventory.requiredCases].sort()
    const appeared = observed.filter(id => !frozen.includes(id))
    const disappeared = frozen.filter(id => !observed.includes(id))
    const verification = {
      schema: 1, kind: "frozen-inventory-verification", library: library.id,
      inventory: { path: relative(root, inventoryPath), ...fileIdentity(inventoryPath) },
      discovery: { path: relative(root, join(directory, "receipt.json")), ...fileIdentity(join(directory, "receipt.json")) },
      frozenCases: frozen.length, observedCases: observed.length, appeared, disappeared,
      suitePassed: receipt.passed, inputsStable: receipt.inputsStable ?? false,
      requiredCases: (receipt.observedCases ?? []).map(row => ({ id: row.id, status: row.status })),
      compiler: receipt.compiler ?? null,
    }
    verification.passed = verification.suitePassed && verification.inputsStable && appeared.length === 0 && disappeared.length === 0
    writeJson(join(directory, "verification.json"), verification)
    results.push({ library: library.id, status: verification.passed ? "verified" : "drifted", appeared: appeared.length, disappeared: disappeared.length, suitePassed: verification.suitePassed })
    process.stderr.write(`${verification.passed ? "VERIFIED" : "DRIFTED"} frozen=${frozen.length} observed=${observed.length} appeared=${appeared.length} disappeared=${disappeared.length}\n`)
  }
  writeJson(join(runDirectory, "summary.json"), { schema: 1, kind: "inventory-verification-batch", runId, results })
  console.log(JSON.stringify(results, null, 1))
}

async function main() {
  const argv = process.argv.slice(2)
  const VALUE_FLAGS = new Set(["--run-id", "--timeout-ms"])
  const flags = new Map(), explicit = []
  for (let at = 0; at < argv.length; at += 1) {
    const value = argv[at]
    if (VALUE_FLAGS.has(value)) { flags.set(value, argv[at + 1]); at += 1 }
    else if (value.startsWith("--")) flags.set(value, true)
    else explicit.push(value)
  }
  const flag = name => (flags.has(name) && flags.get(name) !== true ? flags.get(name) : null)
  const manifest = readJson(manifestPath)
  if (flags.has("--verify")) return verifyFrozen({ manifest, explicit, flag })
  const pending = manifest.libraries.filter(library => !library.tests?.nodeAdapter)
  const selected = explicit.length ? manifest.libraries.filter(library => explicit.includes(library.id)) : pending
  if (!selected.length) { console.error("no libraries selected"); process.exit(2) }
  const runId = flag("--run-id") ?? `${new Date().toISOString().slice(0, 10)}-adapter-inventory`
  const runDirectory = join(root, "benchmarks/migration-results", runId)
  const timeoutMs = Number(flag("--timeout-ms") ?? 300_000)
  const apply = !flags.has("--dry-run")

  const summary = {
    schema: 1, kind: "adapter-inventory-batch", started: new Date().toISOString(), runId,
    selected: selected.map(library => library.id), applied: apply, frozen: [], gaps: [],
  }
  for (const library of selected) {
    const directory = join(runDirectory, library.id)
    process.stderr.write(`\n=== ${library.id} ===\n`)
    const receipt = await discoverAdapter({ library, directory, timeoutMs })
    const row = {
      library: library.id, directory: relative(root, directory), passed: receipt.passed,
      inputsStable: receipt.inputsStable ?? false, cases: receipt.observedCases?.length ?? 0,
      exercised: receipt.exercisedArtifacts ?? [], untested: receipt.untestedEntries ?? [],
      absent: receipt.artifacts?.absent ?? [], deviations: receipt.derivation?.deviations ?? [],
      unsupported: receipt.derivation?.unsupported ?? [], unexpectedErrors: receipt.unexpectedErrors ?? [],
      failure: receipt.failure?.message ?? null,
    }
    if (!receipt.passed) {
      row.reason = receipt.failure?.message ?? receipt.unexpectedErrors?.join("; ") ?? "discovery did not pass"
      row.assignment = receipt.derivation?.unsupported?.length
        ? "adapter cannot be derived from the original command; freeze it by hand or record the runner as an explicit 007/011 task"
        : "original suite does not pass against the existing distribution; this is a required case, not a waiver"
      summary.gaps.push(row)
      process.stderr.write(`FAILED: ${row.reason}\n`)
      continue
    }
    const inventoryName = `${library.id}.required-tests.json`
    const inventory = buildInventory({ library, receipt, reportPath: join(directory, "node/report.json"), workspace: receipt.workspace })
    row.inventory = inventoryName
    row.testCases = inventory.observedInventory.testCases
    row.suiteNodes = inventory.observedInventory.suiteNodes
    if (apply) {
      writeJson(join(root, "benchmarks/libraries", inventoryName), inventory)
      const target = manifest.libraries.find(entry => entry.id === library.id)
      target.tests = {
        ...target.tests,
        adapterStatus: "original-node-command-derived-existing-dist-cases-frozen-source-build-pending",
        requiredCaseInventory: inventoryName,
        nodeAdapter: manifestAdapter({ library, receipt, inventoryName }),
      }
    }
    summary.frozen.push(row)
    process.stderr.write(`frozen: ${inventory.observedInventory.totalIdentities} identities (${inventory.observedInventory.testCases} tests, ${inventory.observedInventory.suiteNodes} suites)\n`)
  }
  if (apply && summary.frozen.length) {
    manifest.inventoryDate = new Date().toISOString().slice(0, 10)
    writeJson(manifestPath, manifest)
  }
  summary.completed = new Date().toISOString()
  summary.manifestSha256 = fileIdentity(manifestPath).sha256
  summary.fingerprint = fingerprint(summary)
  writeJson(join(runDirectory, "summary.json"), summary)
  console.log(JSON.stringify({
    runId, frozen: summary.frozen.map(row => `${row.library}:${row.cases}`), gaps: summary.gaps.map(row => `${row.library}: ${row.reason}`),
  }, null, 1))
  if (summary.gaps.length && flags.has("--strict")) process.exitCode = 1
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main()
