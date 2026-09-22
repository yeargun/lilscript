// Which port configurations the current compiler still accepts.
//
// 001 asks for current and new-route support classified as implemented,
// partial, unsupported or unverified, with real callers attached and gaps
// allocated. A configuration a maintained port ships is a real caller, and a
// field the compiler no longer knows is a support gap that stops that port
// being built from source at all — which is why it belongs in the inventory
// rather than in a port's own bug list.
//
// The probe is deliberately small: compile one trivial source with the port's
// own configuration and read what the compiler says. It classifies the
// configuration, not the port's program.

import { execFileSync } from "node:child_process"
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity, fingerprint } from "./artifact-evidence.mjs"
import { preserveBinary } from "./preserved-binaries.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
const root = resolve(toolsDirectory, "../..")

const PROBE_SOURCE = `export int answer() {\n  return 42;\n}\n`

/** Parse the compiler's own "unknown field" diagnostic. */
export function unknownFields(stderr) {
  return [...stderr.matchAll(/unknown field `([^`]+)`/g)].map(match => match[1])
}

/** Fields the compiler lists as accepted, from the same diagnostic. */
export function acceptedFields(stderr) {
  const match = /expected one of ((?:`[^`]+`(?:, )?)+)/.exec(stderr)
  return match ? [...match[1].matchAll(/`([^`]+)`/g)].map(row => row[1]) : []
}

export function probeConfiguration({ compiler, configuration, scratch, timeoutMs = 20_000 }) {
  const input = join(scratch, "probe.lil")
  const output = join(scratch, "probe.mjs")
  writeFileSync(input, PROBE_SOURCE)
  try {
    execFileSync(compiler, [input, "--target", "js-module", "--config", configuration, "--mode", "development", "-o", output],
      { encoding: "utf8", timeout: timeoutMs, stdio: ["ignore", "pipe", "pipe"] })
    return { status: "accepted", unknownFields: [] }
  } catch (error) {
    const stderr = String(error.stderr ?? error.message ?? "")
    const unknown = unknownFields(stderr)
    if (unknown.length) return { status: "unsupported-field", unknownFields: unknown, accepted: acceptedFields(stderr), diagnostic: stderr.split("\n").slice(0, 6).join("\n") }
    if (/invalid config|TOML parse error/.test(stderr)) return { status: "invalid-configuration", unknownFields: [], diagnostic: stderr.split("\n").slice(0, 6).join("\n") }
    // The configuration was read; the probe program itself is what failed.
    return { status: "accepted-with-compile-error", unknownFields: [], diagnostic: stderr.split("\n").slice(0, 6).join("\n") }
  }
}

/** Every `lilscript*.toml` a maintained boundary declares. */
export function declaredConfigurations({ manifestPath = join(root, "benchmarks/libraries/maintained-workloads.json") } = {}) {
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"))
  const rows = []
  for (const library of [...manifest.libraries, ...(manifest.additionalCoverage ?? [])]) {
    for (const configuration of library.configurations ?? []) {
      rows.push({ boundary: library.id, workspace: library.workspace, path: configuration.path, recordedSha256: configuration.sha256 ?? null })
    }
  }
  return rows
}

export function auditConfigurations({ compiler = join(root, "target/release/lilscript") } = {}) {
  const preserved = preserveBinary(compiler)
  compiler = preserved.path
  const scratch = mkdtempSync("/tmp/lilscript-config-support-")
  const rows = []
  for (const declared of declaredConfigurations()) {
    const configuration = join(declared.workspace, declared.path)
    if (!existsSync(configuration)) { rows.push({ ...declared, status: "missing", unknownFields: [] }); continue }
    const identity = fileIdentity(configuration)
    const drifted = declared.recordedSha256 !== null && declared.recordedSha256 !== identity.sha256
    rows.push({ ...declared, ...identity, drifted, ...probeConfiguration({ compiler, configuration, scratch }) })
  }
  const unsupported = rows.filter(row => row.status === "unsupported-field")
  const fieldUsage = new Map()
  for (const row of unsupported) {
    for (const field of row.unknownFields) {
      if (!fieldUsage.has(field)) fieldUsage.set(field, [])
      fieldUsage.get(field).push(`${row.boundary}/${row.path}`)
    }
  }
  const record = {
    schema: 1, kind: "port-configuration-support-audit", generated: new Date().toISOString(),
    // The audit passes when every declared configuration was probed. Rejected
    // fields are its findings about the ports, not failures of the audit.
    passed: rows.every(row => row.status !== "missing"),
    compiler: preserved,
    counts: {
      configurations: rows.length,
      accepted: rows.filter(row => row.status.startsWith("accepted")).length,
      unsupportedField: unsupported.length,
      invalid: rows.filter(row => row.status === "invalid-configuration").length,
      missing: rows.filter(row => row.status === "missing").length,
      drifted: rows.filter(row => row.drifted).length,
    },
    unsupportedFields: [...fieldUsage.entries()].sort((left, right) => right[1].length - left[1].length),
    rows,
  }
  return record
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  const at = argv.indexOf("--out")
  const record = auditConfigurations()
  record.fingerprint = fingerprint({ ...record, generated: null })
  if (at >= 0) {
    const { mkdirSync } = await import("node:fs")
    const path = resolve(argv[at + 1])
    mkdirSync(dirname(path), { recursive: true })
    writeFileSync(path, JSON.stringify(record, null, 2) + "\n")
  }
  console.log(`${record.counts.configurations} declared configurations: ${record.counts.accepted} accepted, ${record.counts.unsupportedField} name an unsupported field, ${record.counts.invalid} invalid, ${record.counts.missing} missing, ${record.counts.drifted} drifted from the manifest hash`)
  for (const [field, callers] of record.unsupportedFields) {
    console.log(`\n  unsupported field \`${field}\` — ${callers.length} caller(s):`)
    for (const caller of callers) console.log(`    ${caller}`)
  }
}
