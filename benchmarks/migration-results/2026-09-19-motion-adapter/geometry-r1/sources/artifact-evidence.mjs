// Shared build evidence for portgate and fleet consumers. Timings are telemetry;
// actual invocations, immutable inputs and complete output identities are proof.
import { spawnSync } from "node:child_process"
import { createHash, randomUUID } from "node:crypto"
import {
  existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync,
  realpathSync, renameSync, statSync, writeFileSync,
} from "node:fs"
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path"

export const EVIDENCE_SCHEMA = 1
const SHA256 = /^[a-f0-9]{64}$/
export const digest = (bytes) => createHash("sha256").update(bytes).digest("hex")
const canonical = (value) => {
  if (Array.isArray(value)) return value.map(canonical)
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]))
  }
  return value
}
export const fingerprint = (value) => digest(JSON.stringify(canonical(value)))

export function fileIdentity(path) {
  const bytes = readFileSync(path)
  return { sha256: digest(bytes), bytes: bytes.length }
}

function contained(root, path) {
  const part = relative(resolve(root), resolve(path))
  return part !== ".." && !part.startsWith(`..${sep}`) && !isAbsolute(part)
}

// Explicit roots make generated inputs and dependency workspaces visible. This
// deliberately does not use HEAD or git's dirty-file count as a content identity.
export function snapshotInputs(root, { paths = ["."], exclude = [".git", "node_modules", "dist", "target", ".cache"] } = {}) {
  root = realpathSync(root)
  const entries = new Map()
  const excluded = (part) => exclude.some((item) => part === item || part.startsWith(`${item}/`)) || part.split("/").includes("node_modules")
  const visit = (path) => {
    if (!contained(root, path)) throw new Error(`input escapes declared root: ${path}`)
    const name = relative(root, path).split(sep).join("/")
    if (excluded(name)) return
    if (!existsSync(path)) throw new Error(`declared input is missing: ${path}`)
    const info = lstatSync(path)
    if (info.isSymbolicLink()) {
      const target = realpathSync(path)
      if (!statSync(target).isFile()) throw new Error(`directory symlink needs its own input root: ${path}`)
      entries.set(name, { path: name, kind: "symlink", target: readlinkSync(path), ...fileIdentity(target) })
    } else if (info.isDirectory()) {
      for (const child of readdirSync(path).sort()) visit(join(path, child))
    } else if (info.isFile()) {
      entries.set(name, { path: name, kind: "file", executable: Boolean(info.mode & 0o111), ...fileIdentity(path) })
    } else {
      throw new Error(`unsupported input kind: ${path}`)
    }
  }
  for (const path of paths) visit(resolve(root, path))
  const files = [...entries.values()].sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0)
  if (!files.length) throw new Error(`empty input snapshot: ${root}`)
  return { root, files, sha256: fingerprint(files) }
}

export function writeReceipt(path, value) {
  mkdirSync(dirname(path), { recursive: true })
  const temp = `${path}.${randomUUID()}.tmp`
  writeFileSync(temp, `${JSON.stringify(value, null, 2)}\n`)
  renameSync(temp, path)
}

export function outputArgument(args) {
  let output = null
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]
    let value
    if (arg === "-o" || arg === "--output") value = args[++i]
    else if (arg.startsWith("--output=")) value = arg.slice(9)
    else continue
    if (!value || value.startsWith("-") || output !== null) throw new Error("missing or ambiguous compiler output")
    output = value
  }
  if (output === null) throw new Error("compilation receipt requires an explicit output")
  return output
}

function inputFingerprint(files, configuration) {
  const identity = configuration ? { path: configuration.path, sha256: configuration.sha256, bytes: configuration.bytes } : null
  return fingerprint({ files, configuration: identity })
}

function compilationInputSnapshot(cwd, options, args) {
  const snapshot = snapshotInputs(cwd, options)
  let configuration = null
  for (let index = 0; index < args.length; index++) {
    let config
    if (args[index] === "--config") config = args[++index]
    else if (args[index].startsWith("--config=")) config = args[index].slice(9)
    else continue
    if (!config || config.startsWith("-") || configuration) throw new Error("missing or ambiguous configuration input")
    const path = realpathSync(resolve(cwd, config))
    configuration = { path: contained(cwd, path) ? relative(cwd, path) : path, location: path, ...fileIdentity(path) }
  }
  // --config can deliberately point outside a port (the probe's unoptimized
  // control does this). Its actual bytes are inputs even outside the tree.
  return { ...snapshot, configuration, sha256: inputFingerprint(snapshot.files, configuration) }
}

// Called by the compiler wrapper in an isolated arm. Existing output is retained
// in the receipt directory before invoking the compiler, so an exit-zero no-op
// cannot borrow stale bytes. The original library checkout is not a scratch arm.
export function invokeCompiler({ compiler, compilerSha256, args, cwd, receiptDirectory, inputPaths, outputRoots = ["dist"], contractSha256, inputExclude = [] }) {
  if (!SHA256.test(compilerSha256 ?? "") || !SHA256.test(contractSha256 ?? "")) throw new Error("invocation requires compiler and contract identities")
  cwd = realpathSync(cwd)
  receiptDirectory = resolve(receiptDirectory)
  if (contained(cwd, receiptDirectory)) throw new Error("receipt directory must be outside the input workspace")
  const output = resolve(cwd, outputArgument(args))
  const roots = outputRoots.map((root) => resolve(cwd, root))
  if (!roots.length || roots.some((root) => root === cwd || !contained(cwd, root))) throw new Error("artifact roots must be subdirectories of the isolated workspace")
  if (!roots.some((root) => output !== root && contained(root, output))) throw new Error("output is outside the declared artifact roots")
  // Resolve the nearest existing ancestor even when the compiler will create
  // nested directories. No symlink in that ancestry may escape the workspace.
  let existingParent = dirname(output)
  while (!existsSync(existingParent)) existingParent = dirname(existingParent)
  if (!contained(cwd, realpathSync(existingParent))) throw new Error("output parent escapes artifact roots")
  if (existsSync(output) && lstatSync(output).isSymbolicLink()) throw new Error("compiler output cannot be a symlink")
  const actualCompiler = fileIdentity(compiler)
  if (actualCompiler.sha256 !== compilerSha256) throw new Error("compiler identity changed before invocation")
  const snapshotOptions = { paths: inputPaths ?? ["."], exclude: [".git", "node_modules", "target", ".cache", ...outputRoots, ...inputExclude] }
  const before = compilationInputSnapshot(cwd, snapshotOptions, args)
  const id = randomUUID()
  mkdirSync(receiptDirectory, { recursive: true })
  const previous = existsSync(output) ? { ...fileIdentity(output), savedAt: join(receiptDirectory, `${id}.previous-output`) } : null
  if (previous) renameSync(output, previous.savedAt)
  const started = process.hrtime.bigint()
  const result = spawnSync(compiler, args, { cwd, stdio: "inherit", env: process.env })
  const elapsedMs = Number(process.hrtime.bigint() - started) / 1e6
  const receipt = {
    schemaVersion: EVIDENCE_SCHEMA, kind: "compiler-invocation", id,
    compiler: { path: realpathSync(compiler), ...actualCompiler },
    contractSha256, cwd, args, inputSha256: before.sha256,
    inputs: before.files, configuration: before.configuration, output: { path: output, previous },
    exitCode: result.status, signal: result.signal, error: result.error?.message ?? null, elapsedMs,
  }
  try {
    receipt.inputAfterSha256 = compilationInputSnapshot(cwd, snapshotOptions, args).sha256
    receipt.compilerAfterSha256 = fileIdentity(compiler).sha256
    if (existsSync(output) && lstatSync(output).isFile()) Object.assign(receipt.output, fileIdentity(output))
  } catch (error) {
    receipt.error = error.message
  }
  receipt.validationErrors = validateInvocation(receipt, { compilerSha256, contractSha256, verifyOutput: true })
  writeReceipt(join(receiptDirectory, `${id}.json`), receipt)
  return { receipt, exitCode: receipt.validationErrors.length ? (result.status || 1) : 0 }
}

export function validateInvocation(receipt, { compilerSha256, contractSha256, verifyOutput = false } = {}) {
  const errors = []
  if (receipt?.schemaVersion !== EVIDENCE_SCHEMA || receipt?.kind !== "compiler-invocation") return ["unsupported invocation receipt"]
  if (!SHA256.test(compilerSha256 ?? "") || receipt.compiler?.sha256 !== compilerSha256 || receipt.compilerAfterSha256 !== compilerSha256) errors.push("compiler identity mismatch")
  if (!SHA256.test(contractSha256 ?? "") || receipt.contractSha256 !== contractSha256) errors.push("contract identity mismatch")
  if (receipt.exitCode !== 0 || receipt.signal || receipt.error) errors.push("compiler invocation failed")
  if (!Array.isArray(receipt.args) || !receipt.args.length || !receipt.id) errors.push("invocation is incomplete")
  if (!Array.isArray(receipt.inputs) || !receipt.inputs.length || inputFingerprint(receipt.inputs, receipt.configuration) !== receipt.inputSha256) errors.push("input snapshot missing or inconsistent")
  if (receipt.inputSha256 !== receipt.inputAfterSha256) errors.push("inputs changed during compilation")
  if (!SHA256.test(receipt.output?.sha256 ?? "") || !Number.isSafeInteger(receipt.output?.bytes) || receipt.output.bytes < 0) errors.push("compiler produced no measured output")
  if (verifyOutput && SHA256.test(receipt.output?.sha256 ?? "")) {
    try {
      const current = fileIdentity(receipt.output.path)
      if (current.sha256 !== receipt.output.sha256 || current.bytes !== receipt.output.bytes) errors.push("compiler output changed after invocation")
    } catch {
      errors.push("compiler output disappeared")
    }
  }
  return errors
}

export function validateBuild({ exitCode, invocations, compilerSha256, contractSha256, verifyOutputs = true }) {
  const errors = []
  if (exitCode !== 0) errors.push("build command failed")
  if (!Array.isArray(invocations) || !invocations.length) return [...errors, "no compiler invocation receipts"]
  const ids = new Set()
  for (const row of invocations) {
    if (ids.has(row?.id)) errors.push("duplicate compiler invocation receipt")
    ids.add(row?.id)
    errors.push(...validateInvocation(row, { compilerSha256, contractSha256, verifyOutput: verifyOutputs }))
  }
  return errors
}

// Tests must report a fixed required case inventory and which exact production
// artifact hashes they loaded. An exit-zero command alone cannot certify it.
export function validateTests({ requiredCases, cases, exitCode, requiredArtifacts, loadedArtifacts }) {
  const errors = []
  if (exitCode !== 0) errors.push("test command failed")
  if (!Array.isArray(requiredCases) || !requiredCases.length || new Set(requiredCases).size !== requiredCases.length) errors.push("required test inventory missing or ambiguous")
  if (!Array.isArray(cases) || !cases.length) errors.push("executed test report missing")
  const byId = new Map()
  for (const row of Array.isArray(cases) ? cases : []) {
    if (!row?.id || byId.has(row.id)) errors.push("executed case identity missing or duplicated")
    byId.set(row?.id, row)
    if (row?.status !== "pass") errors.push(`test did not pass: ${row?.id}`)
  }
  for (const id of Array.isArray(requiredCases) ? requiredCases : []) if (byId.get(id)?.status !== "pass") errors.push(`required case did not pass: ${id}`)
  if (!Array.isArray(requiredArtifacts) || !requiredArtifacts.length) errors.push("required production artifact inventory missing")
  const loaded = new Set((Array.isArray(loadedArtifacts) ? loadedArtifacts : []).filter((row) => typeof row?.path === "string" && SHA256.test(row.sha256 ?? "")).map((row) => JSON.stringify([resolve(row.path), row.sha256])))
  for (const artifact of Array.isArray(requiredArtifacts) ? requiredArtifacts : []) {
    if (typeof artifact?.path !== "string" || !SHA256.test(artifact.sha256 ?? "") || !loaded.has(JSON.stringify([resolve(artifact.path), artifact.sha256]))) errors.push(`production artifact was not tested: ${artifact?.path}`)
    try {
      if (fileIdentity(artifact.path).sha256 !== artifact.sha256) errors.push(`production artifact changed after testing: ${artifact.path}`)
    } catch {
      errors.push(`production artifact disappeared: ${artifact?.path}`)
    }
  }
  return errors
}

export function validateMeasurements(report, paths) {
  if (report?.schemaVersion !== 1 || !report.codecs || !Array.isArray(report.artifacts)) throw new Error("unsupported codec report")
  const gzip = report.codecs.gzip9
  const brotli = report.codecs.brotli11
  if (gzip?.encoder !== "upstream-stock-zlib-c" || gzip.libraryVersion !== "1.3.1" || gzip.level !== 9 || gzip.mtime !== 0 || brotli?.encoder !== "official-google-brotli-c" || brotli.libraryVersion !== "1.1.0" || brotli.quality !== 11 || brotli.lgwin !== 22 || brotli.mode !== "generic") throw new Error("codec settings differ from the canonical measurement contract")
  const expected = new Set(paths.map((path) => resolve(path)))
  if (expected.size !== paths.length || !paths.length) throw new Error("empty or duplicate measurement inputs")
  const results = new Map()
  for (const row of report.artifacts) {
    if (typeof row?.path !== "string") throw new Error("measured artifact path is missing")
    const path = resolve(row.path)
    if (!expected.has(path) || results.has(path)) throw new Error("unexpected or duplicate measured artifact")
    for (const key of ["raw", "gzip9", "brotli11"]) if (!Number.isSafeInteger(row[key]) || row[key] < 0) throw new Error(`missing or invalid ${key}`)
    if (statSync(path).size !== row.raw) throw new Error("measured raw size does not match the artifact")
    results.set(path, row)
  }
  if (results.size !== expected.size) throw new Error("codec omitted a required artifact")
  return results
}
