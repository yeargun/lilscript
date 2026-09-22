// Freeze one maintained library's original production-test adapter.
//
// 001 asks for every library's original suite, its exact executed cases and the
// artifacts those cases actually loaded, with the inputs proved unchanged by the
// observation. This driver does that for any library whose `npm test` script the
// derivation can account for, so the remaining inventory is a list of runs
// rather than a list of scripts to write.
//
// It never rebuilds, never installs and never edits the original workspace: the
// suite runs against a scratch copy with the original `node_modules` linked in,
// and the run fails if any pinned input moved.

import assert from "node:assert/strict"
import { copyFileSync, cpSync, existsSync, globSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from "node:fs"
import { basename, dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity, fingerprint, snapshotInputs, snapshotDependencyTree } from "./artifact-evidence.mjs"
import { deriveAdapter, reconstruct } from "./adapter-derivation.mjs"
import { runNodeTestEvidence } from "./node-test-evidence.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
const root = resolve(toolsDirectory, "../..")
const WORKSPACE_EXCLUDE = [".git", "node_modules", "target", ".cache"]
const TOOL_NAMES = ["artifact-evidence", "bounded-command", "node-test-evidence", "node-test-reporter", "observe-node-artifacts", "adapter-derivation", "adapter-discovery"]

// Files the original suite is entitled to read but must not change. Everything
// matching this is pinned by identity before, between and after the run.
const FIXTURE_PATTERN = /^(?:test\/|tests\/|site\/|scripts\/|types\/|fixtures\/|package(?:-lock)?\.json$|tsconfig[^/]*\.json$|lilscript[^/]*\.toml$|\.nvmrc$)/

const identify = path => ({ path, ...fileIdentity(path) })

/** Strip every environment variable that could redirect the original suite. */
export function sanitizeEnvironment(env, { node, scratch, userConfig, globalConfig }) {
  const removed = Object.keys(env).filter(key =>
    key.startsWith("LILSCRIPT_") || /^npm_/i.test(key) || ["UPDATE", "NODE_TEST_CONTEXT", "ESBUILD_BINARY_PATH"].includes(key))
  for (const key of removed) delete env[key]
  const overrides = {
    PATH: `${dirname(node)}:/usr/local/bin:/usr/bin:/bin`, NODE_OPTIONS: "", NODE_PATH: "",
    npm_config_offline: "true", npm_config_ignore_scripts: "true", npm_config_audit: "false", npm_config_fund: "false",
    npm_config_userconfig: userConfig, npm_config_globalconfig: globalConfig, npm_config_cache: join(scratch, "npm-cache"),
  }
  Object.assign(env, overrides)
  return { removed, overrides }
}

/**
 * Choose the artifacts whose loads must be observed. `declaredEntryFiles` comes
 * from the frozen workload manifest; only files that exist are pinned, and the
 * absent ones are reported so they stay visible as delivery gaps.
 */
export function selectArtifacts(workspace, declaredEntryFiles) {
  const present = [], absent = []
  for (const declared of declaredEntryFiles ?? []) {
    const relativePath = declared.replace(/^\.\//, "")
    if (existsSync(join(workspace, relativePath))) present.push(relativePath)
    else absent.push(relativePath)
  }
  return { present, absent }
}

/**
 * Run one library's frozen original suite and return its receipt.
 *
 * `library` is a row of `benchmarks/libraries/maintained-workloads.json`.
 * `directory` receives the receipt and is required to be outside the workspace.
 */
export async function discoverAdapter({ library, directory, node = process.execPath, timeoutMs = 300_000, extraLimitations = [] }) {
  const started = performance.now()
  const original = library.workspace
  const npm = join(dirname(node), "npm")
  const scratch = mkdtempSync(`/tmp/lilscript-adapter-${library.id}-`)
  const workspace = join(scratch, basename(original))
  mkdirSync(directory, { recursive: true })
  const save = (name, value) => writeFileSync(join(directory, name), JSON.stringify(value, null, 2) + "\n")

  const userConfig = join(directory, "npm-user.npmrc"), globalConfig = join(directory, "npm-global.npmrc")
  writeFileSync(userConfig, "")
  writeFileSync(globalConfig, "")
  const environment = sanitizeEnvironment(process.env, { node, scratch, userConfig, globalConfig })

  const tools = TOOL_NAMES.map(name => join(toolsDirectory, `${name}.mjs`))
  const npmRoot = dirname(dirname(realpathSync(npm)))
  const pins = () => {
    const files = [node, npm, join(root, ".nvmrc"), userConfig, globalConfig, ...tools].map(identify)
    return { files, sha256: fingerprint(files) }
  }
  const full = target => snapshotInputs(target, { exclude: WORKSPACE_EXCLUDE })
  const capture = () => ({
    inputs: pins(), source: snapshotInputs(original), original: full(original), workspace: full(workspace),
    project: snapshotDependencyTree(join(original, "node_modules")), npm: snapshotDependencyTree(npmRoot),
  })

  const receipt = {
    schema: 1, kind: "existing-dist-original-suite-discovery", library: library.id,
    started: new Date().toISOString(), passed: false, qualification: "unverified", sourceBuilt: false,
    original, workspace, node: identify(node), boundsMs: { suite: timeoutMs },
    environment: { ...environment, updateAbsent: !("UPDATE" in process.env), otherwise: "inherited; original workspace npm configuration preserved" },
    limitations: [
      "Existing distribution only: no compiler execution, no source-built qualification, no installation and no codec measurement.",
      "Case identities come from the original suite as it runs today. A case this compiler cannot pass is still required; it is recorded, not removed.",
      "Artifact observation covers the files the manifest declares as entry points and that exist. Absent entries are reported as delivery gaps, not waived.",
      "Installed project and global npm contents are pinned. The wider operating-system environment is not.",
      ...extraLimitations,
    ],
  }
  let before
  save("receipt.json", receipt)
  try {
    const packageJson = JSON.parse(readFileSync(join(original, "package.json"), "utf8"))
    const script = packageJson.scripts?.test
    receipt.originalTestScript = script
    const derivation = deriveAdapter(script)
    receipt.derivation = derivation
    assert.equal(reconstruct(derivation), splitNormalize(script), "derivation lost part of the original test script")
    assert(derivation.supported, `original test script is not a plain node --test command: ${derivation.unsupported.join("; ")}`)

    assert.equal(process.version, `v${readFileSync(join(root, ".nvmrc"), "utf8").trim().replace(/^v/, "")}`, "discovery must use the repository-pinned Node version")
    assert.deepEqual(fileIdentity(process.execPath), fileIdentity(node), "discovery must run on the pinned Node binary")

    const artifacts = selectArtifacts(original, library.package?.declaredEntryFiles)
    receipt.artifacts = artifacts
    assert(artifacts.present.length, "no declared entry file exists in the workspace")

    cpSync(original, workspace, { recursive: true, filter: path => !WORKSPACE_EXCLUDE.includes(relative(original, path).split("/")[0]) })
    if (existsSync(join(original, "node_modules"))) symlinkSync(join(original, "node_modules"), join(workspace, "node_modules"), "dir")
    before = capture()
    assert.deepEqual(before.workspace.files, before.original.files, "scratch copy differs from the original workspace")
    save("before.json", before)

    const fixtures = before.source.files.filter(row => FIXTURE_PATTERN.test(row.path)).map(({ path, sha256, bytes }) => ({ path, sha256, bytes }))
    const npmIdentity = identify(npm)
    const prerequisites = derivation.prerequisites.map(prerequisite => ({
      id: prerequisite.id, executable: npmIdentity, args: ["run", prerequisite.script], timeoutMs: Math.min(timeoutMs, 180_000),
      inputs: ["package.json", "package-lock.json"].filter(name => existsSync(join(original, name)))
        .map(name => ({ path: name, ...fileIdentity(join(original, name)) })),
    }))
    save("declarations.json", { patterns: derivation.patterns, nodeArguments: derivation.nodeArguments, artifacts: artifacts.present, fixtures, prerequisites })
    copyFileSync(fileURLToPath(import.meta.url), join(directory, "discovery-source.mjs"))
    mkdirSync(join(directory, "runner-sources"))
    for (const path of tools) copyFileSync(path, join(directory, "runner-sources", basename(path)))

    const remaining = Math.floor(timeoutMs - (performance.now() - started))
    assert(remaining > 0, "discovery budget exhausted before the Node owner started")
    const report = await runNodeTestEvidence({
      cwd: workspace, files: expandOriginalSelection(workspace, derivation.patterns), requiredFilePatterns: derivation.patterns,
      nodeArguments: derivation.nodeArguments, artifactPaths: artifacts.present, requiredCases: [],
      requiredTestFiles: null, requiredFixtures: fixtures, prerequisites, directory: join(directory, "node"), timeoutMs: remaining,
    })
    receipt.report = identify(join(directory, "node/report.json"))
    receipt.observedCases = report.evidence.cases
    receipt.loadedArtifacts = report.evidence.loadedArtifacts.map(({ path, sha256, bytes }) => ({ path, sha256, bytes }))
    receipt.prerequisites = report.prerequisites?.map(row => ({ id: row.declaration.id, attempted: row.attempted, status: row.status }))
    receipt.discoveryErrors = report.errors
    // Discovery is the run that *creates* the inventory, so the missing-inventory
    // sentinel is expected. A declared entry the original suite never loads is a
    // real delivery gap: record it as an unexercised boundary rather than
    // dropping it from the manifest or letting it fail the observation.
    const untestedPrefix = "production artifact was not tested: "
    const untested = report.errors.filter(line => line.startsWith(untestedPrefix)).map(line => relative(workspace, line.slice(untestedPrefix.length)))
    const unexpected = report.errors.filter(line => line !== "required test inventory missing or ambiguous" && !line.startsWith(untestedPrefix))
    receipt.untestedEntries = untested
    receipt.unexpectedErrors = unexpected
    receipt.exercisedArtifacts = artifacts.present.filter(path => !untested.includes(path))
    receipt.passed = report.suiteExecuted && report.evidence.exitCode === 0 && report.evidence.cases.length > 0 &&
      report.evidence.cases.every(row => row.status === "pass") &&
      receipt.exercisedArtifacts.length > 0 && unexpected.length === 0
  } catch (error) {
    receipt.failure = { message: error.message, stack: error.stack }
  } finally {
    try {
      const after = capture()
      save("after.json", after)
      assert(before, "initial snapshot incomplete")
      assert.deepEqual(after, before, "discovery inputs changed")
      receipt.inputsStable = true
    } catch (error) {
      receipt.passed = false
      receipt.inputsStable = false
      receipt.validationFailure = { message: error.message, stack: error.stack }
    }
    receipt.elapsedMs = performance.now() - started
    receipt.completed = new Date().toISOString()
    receipt.outputs = snapshotInputs(directory, { exclude: ["receipt.json"] }).files
    save("receipt.json", receipt)
  }
  return receipt
}

// `npm` collapses runs of whitespace inside a script the same way the shell
// does; normalizing both sides keeps the reconstruction check about content.
function splitNormalize(script) {
  return script.split("&&").map(part => part.trim()).filter(Boolean).join(" && ")
}

function expandOriginalSelection(cwd, patterns) {
  // `expandTestSelection` in node-test-evidence.mjs owns the same expansion and
  // re-checks it during the run; this call only orders the entry files.
  return [...new Set(globSync(patterns, { cwd }))].sort()
}

/** Load one library row from the frozen workload manifest. */
export function libraryRow(id) {
  const manifest = JSON.parse(readFileSync(join(root, "benchmarks/libraries/maintained-workloads.json"), "utf8"))
  const rows = [...manifest.libraries, ...(manifest.additionalCoverage ?? [])]
  const row = rows.find(entry => entry.id === id)
  if (!row) throw new Error(`no workload row named ${id}`)
  return row
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [id, ...rest] = process.argv.slice(2)
  if (!id) { console.error("usage: adapter-discovery.mjs <library-id> [--out <directory>] [--timeout-ms <n>]"); process.exit(2) }
  const flag = name => { const at = rest.indexOf(name); return at < 0 ? null : rest[at + 1] }
  const directory = resolve(flag("--out") ?? join(root, `benchmarks/migration-results/${new Date().toISOString().slice(0, 10)}-adapters/${id}`))
  const timeoutMs = Number(flag("--timeout-ms") ?? 300_000)
  const receipt = await discoverAdapter({ library: libraryRow(id), directory, timeoutMs })
  console.log(JSON.stringify({
    library: id, directory, passed: receipt.passed, inputsStable: receipt.inputsStable,
    cases: receipt.observedCases?.length ?? 0, loaded: receipt.loadedArtifacts?.length ?? 0,
    deviations: receipt.derivation?.deviations ?? [], unsupported: receipt.derivation?.unsupported ?? [],
    exercised: receipt.exercisedArtifacts, untested: receipt.untestedEntries, absent: receipt.artifacts?.absent,
    unexpectedErrors: receipt.unexpectedErrors, failure: receipt.failure?.message,
  }, null, 1))
  if (!receipt.passed) process.exitCode = 1
}
