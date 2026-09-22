import { existsSync, globSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { digest, fileIdentity, validateTests, writeReceipt } from "./artifact-evidence.mjs"
import { runBoundedCommand } from "./bounded-command.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))

function validatePrerequisites(prerequisites, cwd) {
  if (!Array.isArray(prerequisites)) throw new Error("prerequisites must be an array")
  const ids = new Set(), commands = new Set()
  const validText = value => typeof value === "string" && value.trim() && !value.includes("\0")
  const validIdentity = value => value && validText(value.path) && /^[a-f0-9]{64}$/.test(value.sha256 ?? "") && Number.isSafeInteger(value.bytes) && value.bytes >= 0
  for (const prerequisite of prerequisites) {
    if (!prerequisite || !validText(prerequisite.id) || ids.has(prerequisite.id)) throw new Error("prerequisite IDs must be nonempty and unique")
    ids.add(prerequisite.id)
    if (!validIdentity(prerequisite.executable) || !Array.isArray(prerequisite.args) || !prerequisite.args.length || prerequisite.args.some(value => typeof value !== "string" || value.includes("\0"))) throw new Error("prerequisite needs a pinned executable and explicit argv")
    if (!Number.isSafeInteger(prerequisite.timeoutMs) || prerequisite.timeoutMs <= 0 || prerequisite.timeoutMs > 2147483647) throw new Error("prerequisite timeout must be a positive bounded integer")
    if (!Array.isArray(prerequisite.inputs) || !prerequisite.inputs.length || prerequisite.inputs.some(value => !validIdentity(value))) throw new Error("prerequisite inputs must be nonempty pinned identities")
    if (new Set(prerequisite.inputs.map(value => resolve(cwd, value.path))).size !== prerequisite.inputs.length) throw new Error("prerequisite inputs must be unique")
    const command = JSON.stringify([resolve(cwd, prerequisite.executable.path), prerequisite.args])
    if (commands.has(command)) throw new Error("prerequisite executable and argv must be unique")
    commands.add(command)
  }
}

function prerequisiteInputs(cwd, prerequisite) {
  return pinnedInputs(cwd, [prerequisite.executable, ...prerequisite.inputs])
}

function pinnedInputs(cwd, inputs) {
  return inputs.map(expected => {
    const path = resolve(cwd, expected.path)
    try {
      const actual = fileIdentity(path)
      return { path, expected, actual, matches: actual.sha256 === expected.sha256 && actual.bytes === expected.bytes }
    } catch (error) { return { path, expected, matches: false, error: error.message } }
  })
}

export function expandTestSelection(cwd, patterns) {
  if (!Array.isArray(patterns) || !patterns.length || patterns.some(pattern => typeof pattern !== "string" || !pattern)) throw new Error("original test selection patterns must be explicit")
  return [...new Set(globSync(patterns, { cwd }))].sort()
}

export async function runNodeTestEvidence({ cwd, files, artifactPaths, requiredCases, requiredTestFiles, requiredFilePatterns, requiredFixtures = [], prerequisites = [], directory, node = process.execPath, nodeArguments = [], timeoutMs = 2700000 }) {
  cwd = resolve(cwd)
  directory = resolve(directory)
  validatePrerequisites(prerequisites, cwd)
  if (!Array.isArray(requiredFixtures) || requiredFixtures.some(input => !input || typeof input.path !== "string" || !input.path.trim() || input.path.includes("\0") || !/^[a-f0-9]{64}$/.test(input.sha256 ?? "") || !Number.isSafeInteger(input.bytes) || input.bytes < 0)
    || new Set(requiredFixtures.map(input => resolve(cwd, input.path))).size !== requiredFixtures.length) throw new Error("required fixtures must have unique paths and pinned identities")
  const part = relative(cwd, directory)
  if (part !== ".." && !part.startsWith("../") && !isAbsolute(part)) throw new Error("test evidence must be outside the input workspace")
  if (existsSync(directory)) throw new Error("test evidence directory already exists")
  if (!Array.isArray(files) || !files.length || !Array.isArray(artifactPaths) || !artifactPaths.length) throw new Error("test files and production artifacts must be explicit")
  mkdirSync(directory, { recursive: true })
  const requiredArtifacts = artifactPaths.map((path) => ({ path: resolve(cwd, path), ...fileIdentity(resolve(cwd, path)) }))
  const testFiles = files.map((path) => ({ path, ...fileIdentity(resolve(cwd, path)) }))
  const toolInputs = ["node-test-evidence.mjs", "node-test-reporter.mjs", "observe-node-artifacts.mjs", "artifact-evidence.mjs", "bounded-command.mjs"].map(name => ({ path: join(toolsDirectory, name), ...fileIdentity(join(toolsDirectory, name)) }))
  const runtime = { path: node, ...fileIdentity(node) }
  const version = await runBoundedCommand(node, ["--version"], { encoding: "utf8", timeoutMs: Math.min(timeoutMs, 10000) })
  runtime.version = version.stdout.trim()
  runtime.supervision = version.supervision
  const selectedFiles = requiredFilePatterns ? expandTestSelection(cwd, requiredFilePatterns) : null
  const loadedDirectory = join(directory, "loaded")
  const preload = join(directory, "observe.mjs")
  writeFileSync(preload, `import { observeNodeArtifacts } from ${JSON.stringify(pathToFileURL(join(toolsDirectory, "observe-node-artifacts.mjs")).href)};\nobserveNodeArtifacts(${JSON.stringify({ paths: requiredArtifacts.map((artifact) => artifact.path), directory: loadedDirectory })});\n`)
  const eventsPath = join(directory, "events.jsonl")
  const args = [...nodeArguments, "--test", "--test-reporter", join(toolsDirectory, "node-test-reporter.mjs"), "--test-reporter-destination", eventsPath, ...files]
  // This starts an independent test runner, even when called from a tool test.
  // Inheriting Node's private child-runner context suppresses normal reporting.
  const childEnvironment = { ...process.env }
  delete childEnvironment.NODE_TEST_CONTEXT
  // Node propagates NODE_OPTIONS to child processes started by library tests.
  // A file URL needs no shell quoting and cannot turn spaces into extra flags.
  const inheritedNodeOptions = childEnvironment.NODE_OPTIONS ?? ""
  const prerequisiteEnvironment = { ...childEnvironment, PATH: `${dirname(resolve(node))}:${childEnvironment.PATH ?? ""}` }
  const fixtureInputs = { before: pinnedInputs(cwd, requiredFixtures) }
  const fixtureErrors = inputs => inputs.filter(input => !input.matches).map(input => `required fixture differs: ${input.expected.path}`)
  const preflightErrors = fixtureErrors(fixtureInputs.before)
  const prerequisiteReports = [], prerequisiteErrors = []
  for (const [index, prerequisite] of prerequisites.entries()) {
    const row = { declaration: prerequisite, command: resolve(cwd, prerequisite.executable.path), args: prerequisite.args, attempted: false, status: null, before: prerequisiteInputs(cwd, prerequisite) }
    prerequisiteReports.push(row)
    if (preflightErrors.length) { row.notExecuted = "required fixture differs before execution"; continue }
    if (prerequisiteErrors.length) { row.notExecuted = "earlier prerequisite did not pass"; continue }
    if (row.before.some(input => !input.matches)) {
      row.notExecuted = "pinned prerequisite input differs"
      prerequisiteErrors.push(`prerequisite ${prerequisite.id} input differs before execution`)
      continue
    }
    row.attempted = true
    const commandResult = await runBoundedCommand(row.command, row.args, { cwd, env: prerequisiteEnvironment, encoding: "utf8", timeoutMs: Math.min(timeoutMs, prerequisite.timeoutMs), maxBuffer: 1 << 28 })
    const stdout = join(directory, `prerequisite-${index}.stdout`), stderr = join(directory, `prerequisite-${index}.stderr`)
    writeFileSync(stdout, commandResult.stdout ?? "")
    writeFileSync(stderr, commandResult.stderr ?? "")
    Object.assign(row, { status: commandResult.status, signal: commandResult.signal, error: commandResult.error?.message ?? null, supervision: commandResult.supervision, stdout: { path: stdout, ...fileIdentity(stdout) }, stderr: { path: stderr, ...fileIdentity(stderr) }, after: prerequisiteInputs(cwd, prerequisite) })
    if (commandResult.status !== 0 || commandResult.error || commandResult.signal) prerequisiteErrors.push(`prerequisite ${prerequisite.id} did not complete successfully`)
    if (row.after.some(input => !input.matches)) prerequisiteErrors.push(`prerequisite ${prerequisite.id} input changed during execution`)
  }
  childEnvironment.NODE_OPTIONS = `${inheritedNodeOptions} --import=${pathToFileURL(preload).href}`.trim()
  fixtureInputs.afterPrerequisites = pinnedInputs(cwd, requiredFixtures)
  preflightErrors.push(...fixtureErrors(fixtureInputs.afterPrerequisites))
  const suiteExecuted = prerequisiteErrors.length === 0 && preflightErrors.length === 0
  const result = suiteExecuted
    ? await runBoundedCommand(node, args, { cwd, env: childEnvironment, encoding: "utf8", timeoutMs, maxBuffer: 1 << 28 })
    : { status: null, signal: null, error: null, stdout: "", stderr: "", supervision: null }
  writeFileSync(join(directory, "runner.log"), `${result.stdout ?? ""}\n${result.stderr ?? ""}`)
  const events = existsSync(eventsPath) ? readFileSync(eventsPath, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)) : []
  const cases = events.filter((row) => row.kind === "test-case")
  const loadedArtifacts = existsSync(loadedDirectory) ? readdirSync(loadedDirectory).filter((name) => name.endsWith(".json")).sort().map((name) => JSON.parse(readFileSync(join(loadedDirectory, name), "utf8"))) : []
  const evidence = { exitCode: result.status, requiredCases: requiredCases ?? [], cases, requiredArtifacts, loadedArtifacts }
  const errors = [...preflightErrors, ...prerequisiteErrors, ...validateTests(evidence)]
  if (!suiteExecuted) errors.push(preflightErrors.length ? "Node suite was not executed because a pinned fixture differs" : "Node suite was not executed because a prerequisite did not pass")
  for (const row of prerequisiteReports) {
    row.final = prerequisiteInputs(cwd, row.declaration)
    if (row.final.some(input => !input.matches)) errors.push(`prerequisite ${row.declaration.id} input differs after test execution`)
  }
  if (version.status !== 0 || version.error || version.signal) errors.push("Node runtime version probe did not complete")
  if (result.error || result.signal) errors.push(`Node process did not complete: ${result.error?.message ?? result.signal}`)
  if (selectedFiles && JSON.stringify(selectedFiles) !== JSON.stringify([...files].sort())) errors.push("original test selection differs from frozen entry files")
  if (selectedFiles && JSON.stringify(expandTestSelection(cwd, requiredFilePatterns)) !== JSON.stringify(selectedFiles)) errors.push("original test selection changed during execution")
  if (requiredTestFiles) {
    const expected = new Map(requiredTestFiles.map((file) => [file.path, file.sha256]))
    if (expected.size !== requiredTestFiles.length || expected.size !== testFiles.length) errors.push("required test file inventory differs")
    for (const file of testFiles) if (expected.get(file.path) !== file.sha256) errors.push(`required test source differs: ${file.path}`)
  }
  fixtureInputs.final = pinnedInputs(cwd, requiredFixtures)
  errors.push(...fixtureErrors(fixtureInputs.final))
  for (const file of testFiles) {
    if (!existsSync(resolve(cwd, file.path)) || fileIdentity(resolve(cwd, file.path)).sha256 !== file.sha256) errors.push(`test source changed during execution: ${file.path}`)
  }
  for (const input of [...toolInputs, runtime]) if (!existsSync(input.path) || fileIdentity(input.path).sha256 !== input.sha256) errors.push(`evidence tool or runtime changed during execution: ${input.path}`)
  const report = {
    schemaVersion: 1, kind: "node-production-tests", cwd, args, testFiles,
    selection: requiredFilePatterns ? { patterns: requiredFilePatterns, files: selectedFiles } : null,
    observer: { preload, ...fileIdentity(preload), inheritedNodeOptionsSha256: digest(inheritedNodeOptions) },
    runtime, toolInputs,
    suiteExecuted,
    ...(requiredFixtures.length ? { fixtureInputs } : {}),
    ...(prerequisites.length ? { prerequisites: prerequisiteReports, prerequisiteEnvironment: { PATH: prerequisiteEnvironment.PATH } } : {}),
    supervision: result.supervision,
    evidence, errors, certification: errors.length ? "unverified" : "verified",
    // A failing or skipped declaration is still required. Inventory discovery
    // must never hide it merely because the current compiler cannot pass it.
    inventoryCandidate: cases.map((row) => row.id),
  }
  writeReceipt(join(directory, "report.json"), report)
  return report
}
