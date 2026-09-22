import { existsSync, globSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { digest, fileIdentity, validateTests, writeReceipt } from "./artifact-evidence.mjs"
import { runBoundedCommand } from "./bounded-command.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))

export function expandTestSelection(cwd, patterns) {
  if (!Array.isArray(patterns) || !patterns.length || patterns.some(pattern => typeof pattern !== "string" || !pattern)) throw new Error("original test selection patterns must be explicit")
  return [...new Set(globSync(patterns, { cwd }))].sort()
}

export async function runNodeTestEvidence({ cwd, files, artifactPaths, requiredCases, requiredTestFiles, requiredFilePatterns, requiredFixtures = [], directory, node = process.execPath, nodeArguments = [], timeoutMs = 2700000 }) {
  cwd = resolve(cwd)
  directory = resolve(directory)
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
  childEnvironment.NODE_OPTIONS = `${inheritedNodeOptions} --import=${pathToFileURL(preload).href}`.trim()
  const result = await runBoundedCommand(node, args, { cwd, env: childEnvironment, encoding: "utf8", timeoutMs, maxBuffer: 1 << 28 })
  writeFileSync(join(directory, "runner.log"), `${result.stdout ?? ""}\n${result.stderr ?? ""}`)
  const events = existsSync(eventsPath) ? readFileSync(eventsPath, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)) : []
  const cases = events.filter((row) => row.kind === "test-case")
  const loadedArtifacts = existsSync(loadedDirectory) ? readdirSync(loadedDirectory).filter((name) => name.endsWith(".json")).sort().map((name) => JSON.parse(readFileSync(join(loadedDirectory, name), "utf8"))) : []
  const evidence = { exitCode: result.status, requiredCases: requiredCases ?? [], cases, requiredArtifacts, loadedArtifacts }
  const errors = validateTests(evidence)
  if (version.status !== 0 || version.error || version.signal) errors.push("Node runtime version probe did not complete")
  if (result.error || result.signal) errors.push(`Node process did not complete: ${result.error?.message ?? result.signal}`)
  if (selectedFiles && JSON.stringify(selectedFiles) !== JSON.stringify([...files].sort())) errors.push("original test selection differs from frozen entry files")
  if (selectedFiles && JSON.stringify(expandTestSelection(cwd, requiredFilePatterns)) !== JSON.stringify(selectedFiles)) errors.push("original test selection changed during execution")
  if (requiredTestFiles) {
    const expected = new Map(requiredTestFiles.map((file) => [file.path, file.sha256]))
    if (expected.size !== requiredTestFiles.length || expected.size !== testFiles.length) errors.push("required test file inventory differs")
    for (const file of testFiles) if (expected.get(file.path) !== file.sha256) errors.push(`required test source differs: ${file.path}`)
  }
  for (const fixture of requiredFixtures) {
    const path = resolve(cwd, fixture.path)
    if (!existsSync(path) || fileIdentity(path).sha256 !== fixture.sha256) errors.push(`required fixture differs: ${fixture.path}`)
  }
  for (const file of testFiles) {
    if (!existsSync(resolve(cwd, file.path)) || fileIdentity(resolve(cwd, file.path)).sha256 !== file.sha256) errors.push(`test source changed during execution: ${file.path}`)
  }
  for (const input of [...toolInputs, runtime]) if (!existsSync(input.path) || fileIdentity(input.path).sha256 !== input.sha256) errors.push(`evidence tool or runtime changed during execution: ${input.path}`)
  const report = {
    schemaVersion: 1, kind: "node-production-tests", cwd, args, testFiles,
    selection: requiredFilePatterns ? { patterns: requiredFilePatterns, files: selectedFiles } : null,
    observer: { preload, ...fileIdentity(preload), inheritedNodeOptionsSha256: digest(inheritedNodeOptions) },
    runtime, toolInputs,
    supervision: result.supervision,
    evidence, errors, certification: errors.length ? "unverified" : "verified",
    // A failing or skipped declaration is still required. Inventory discovery
    // must never hide it merely because the current compiler cannot pass it.
    inventoryCandidate: cases.map((row) => row.id),
  }
  writeReceipt(join(directory, "report.json"), report)
  return report
}
