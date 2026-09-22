// Negative probes for the migration progress validator.
//
// 001 requires the validator to reject stale artifacts, missing cases, changed
// installed dependencies or configuration, tampered hashes, empty competitor
// sets and checked milestones without accepted evidence. Each probe below builds
// a claim that should be refused and asserts the exact reason, so a validator
// that stops checking one of them fails here rather than passing silently.

import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { digest } from "./artifact-evidence.mjs"
import { parsePlanTable, validateProgress, validateReceipt } from "./migration-progress.mjs"

const STEPS = [
  ["001", "Baselines and support inventory", "None"],
  ["002", "Language and public boundaries", "001 examples and support map"],
  ["003", "Policy and resource ownership", "001, 002"],
]

function plan(rows) {
  const body = rows.map(([step, title, prerequisites, done, state, receipt]) =>
    `| ${done ? "[x]" : "[ ]"} | [${step} ${title}](#${step}-slug) | ${prerequisites} | ${state} | ${receipt} |`).join("\n")
  return `# Compiler Migration Plan\n\n## Progress and Dependencies\n\n| Done | Step | Prerequisites for closure | State | Accepted receipt |\n|---|---|---|---|---|\n${body}\n`
}

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "lilscript-migration-progress-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(root, "benchmarks/migration-results/run"), { recursive: true })
  const write = (relative, value) => {
    const path = join(root, relative)
    mkdirSync(join(path, ".."), { recursive: true })
    writeFileSync(path, typeof value === "string" ? value : JSON.stringify(value, null, 2))
    return path
  }
  const identity = value => {
    const text = typeof value === "string" ? value : JSON.stringify(value, null, 2)
    return { sha256: digest(Buffer.from(text)), bytes: Buffer.byteLength(text) }
  }
  return { root, write, identity }
}

// A receipt that supports everything a closure claim can ask of it.
function goodReceipt(extra = {}) {
  return {
    schema: 1, kind: "probe", passed: true, inputsStable: true, sourceBuilt: true,
    requiredCases: [{ id: "case-1", status: "pass" }],
    competitors: [{ id: "terser@5.44.0", sha256: "a".repeat(64) }],
    measurements: { raw: 1000, gzip: 400, brotli: 350 },
    ...extra,
  }
}

function acceptedLedger(entry) {
  return { schemaVersion: 1, milestones: { "001": { state: "verified", receipts: [entry] } } }
}

test("a complete claim with supporting evidence is accepted", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt()
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const ledger = acceptedLedger({ path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: ["cases", "competitors", "sourceBuilt", "measurements"] })
  const markdown = plan([[...STEPS[0], true, "verified", "run/receipt.json"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger })
  assert.deepEqual(result.findings, [])
  assert.equal(result.ok, true)
})

test("a checked milestone without accepted evidence is refused", t => {
  const { root } = fixture(t)
  const markdown = plan([[...STEPS[0], true, "verified", "None"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger: { schemaVersion: 1, milestones: {} } })
  assert(result.findings.some(finding => finding.includes("checked without accepted evidence")), result.findings.join("\n"))
  assert(result.findings.some(finding => finding.includes("the plan names no accepted receipt")), result.findings.join("\n"))
})

test("a tampered receipt hash is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt()
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const pinned = identity(receipt)
  // The receipt on disk now says something else than the ledger accepted.
  write("benchmarks/migration-results/run/receipt.json", goodReceipt({ measurements: { raw: 1, gzip: 1, brotli: 1 } }))
  const ledger = acceptedLedger({ path: "benchmarks/migration-results/run/receipt.json", ...pinned, requires: [] })
  const markdown = plan([[...STEPS[0], true, "verified", "run/receipt.json"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger })
  assert(result.findings.some(finding => finding.includes("content changed since it was recorded")), result.findings.join("\n"))
})

test("a stale artifact recorded by the receipt is refused", t => {
  const { root, write, identity } = fixture(t)
  write("dist/library.js", "export const version = 1\n")
  const receipt = goodReceipt({ requiredArtifacts: [{ path: "dist/library.js", sha256: digest(Buffer.from("export const version = 1\n")), bytes: 25 }] })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  write("dist/library.js", "export const version = 2\n")
  const ledger = acceptedLedger({ path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: [] })
  const markdown = plan([[...STEPS[0], true, "verified", "run/receipt.json"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger })
  assert(result.findings.some(finding => finding.includes("artifacts: content changed since it was recorded: dist/library.js")), result.findings.join("\n"))
})

test("a changed installed dependency or configuration is refused", t => {
  const { root, write, identity } = fixture(t)
  const installed = `{"version":"5.9.0"}`, configuration = "[javascript]\ncost_model=\"brotli\"\n"
  write("node_modules/typescript/package.json", installed)
  write("lilscript.toml", configuration)
  const pin = (path, text) => ({ path, sha256: digest(Buffer.from(text)), bytes: Buffer.byteLength(text) })
  const receipt = goodReceipt({
    inputs: { files: [pin("node_modules/typescript/package.json", installed), pin("lilscript.toml", configuration)] },
  })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  write("node_modules/typescript/package.json", `{"version":"5.9.3"}`)
  const ledger = acceptedLedger({ path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: [] })
  const markdown = plan([[...STEPS[0], true, "verified", "run/receipt.json"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger })
  assert(result.findings.some(finding => finding.includes("node_modules/typescript/package.json")), result.findings.join("\n"))
  assert(!result.findings.some(finding => finding.includes("lilscript.toml")), "the unchanged configuration must not be reported")
})

test("a claim of case coverage with no cases is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt({ requiredCases: [] })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const findings = validateReceipt({ root, label: "001", entry: { path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: ["cases"] } })
  assert(findings.some(finding => finding.includes("records no cases")), findings.join("\n"))
})

test("a case that did not pass is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt({ requiredCases: [{ id: "case-1", status: "pass" }, { id: "case-2", status: "fail" }] })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const findings = validateReceipt({ root, label: "001", entry: { path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: ["cases"] } })
  assert(findings.some(finding => finding.includes("records a case that did not pass")), findings.join("\n"))
})

test("an empty competitor set is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt({ competitors: [] })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const findings = validateReceipt({ root, label: "013", entry: { path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: ["competitors"] } })
  assert(findings.some(finding => finding.includes("names no eligible competitor")), findings.join("\n"))
})

test("a competitor with no pinned artifact identity is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt({ competitors: [{ id: "terser@5.44.0" }] })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const findings = validateReceipt({ root, label: "013", entry: { path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: ["competitors"] } })
  assert(findings.some(finding => finding.includes("no pinned artifact identity")), findings.join("\n"))
})

test("a receipt that is not a source build cannot be accepted as one", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt({ sourceBuilt: false })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const findings = validateReceipt({ root, label: "001", entry: { path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: ["sourceBuilt"] } })
  assert(findings.some(finding => finding.includes("sourceBuilt=false")), findings.join("\n"))
})

test("a receipt whose own inputs moved while it ran is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt({ inputsStable: false })
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const findings = validateReceipt({ root, label: "001", entry: { path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: [] } })
  assert(findings.some(finding => finding.includes("inputs changed while it ran")), findings.join("\n"))
})

test("a checked milestone whose prerequisite is open is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt()
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const entry = { path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: [] }
  const ledger = { schemaVersion: 1, milestones: { "001": { receipts: [entry] }, "003": { receipts: [entry] } } }
  const markdown = plan([[...STEPS[0], true, "verified", "run/receipt.json"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], true, "verified", "run/receipt.json"]])
  const result = validateProgress({ root, markdown, ledger })
  assert(result.findings.some(finding => finding === "003: checked while prerequisite 002 is not"), result.findings.join("\n"))
})

test("a state the plan does not declare is refused", t => {
  const { root } = fixture(t)
  const markdown = plan([[...STEPS[0], false, "nearly done", "None"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger: { schemaVersion: 1, milestones: {} } })
  assert(result.findings.some(finding => finding.includes("is not one of the declared states")), result.findings.join("\n"))
})

test("evidence accepted for a step the plan does not declare is refused", t => {
  const { root, write, identity } = fixture(t)
  const receipt = goodReceipt()
  write("benchmarks/migration-results/run/receipt.json", receipt)
  const ledger = { schemaVersion: 1, milestones: { "099": { receipts: [{ path: "benchmarks/migration-results/run/receipt.json", ...identity(receipt), requires: [] }] } } }
  const markdown = plan([[...STEPS[0], false, "active", "None"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger })
  assert(result.findings.some(finding => finding.includes("which the plan does not declare")), result.findings.join("\n"))
})

test("an accepted receipt that does not exist is refused", t => {
  const { root } = fixture(t)
  const ledger = acceptedLedger({ path: "benchmarks/migration-results/run/absent.json", sha256: "b".repeat(64), bytes: 10, requires: [] })
  const markdown = plan([[...STEPS[0], true, "verified", "run/absent.json"], [...STEPS[1], false, "ready", "None"], [...STEPS[2], false, "waiting", "None"]])
  const result = validateProgress({ root, markdown, ledger })
  assert(result.findings.some(finding => finding.includes("accepted receipt does not exist")), result.findings.join("\n"))
})

test("a progress table that cannot be read is an error, not a pass", t => {
  const { root } = fixture(t)
  assert.throws(() => validateProgress({ root, markdown: "# Plan\n\nno table here\n", ledger: { milestones: {} } }), /no progress table/)
  assert.throws(() => parsePlanTable("| Done | Step | Prerequisites for closure | State | Accepted receipt |\n|---|---|---|---|---|\n| [?] | [001 x](#a) | None | ready | None |\n"), /unreadable checkbox/)
  assert.throws(() => parsePlanTable("| Done | Step | Prerequisites for closure | State | Accepted receipt |\n|---|---|---|---|---|\n| [ ] | not a step | None | ready | None |\n"), /does not name a numbered step/)
})

test("the repository's own plan parses and every declared step is present", () => {
  const result = validateProgress()
  assert.equal(result.rows.length, 14)
  assert.deepEqual(result.rows.map(row => row.step), Array.from({ length: 14 }, (_, index) => String(index + 1).padStart(3, "0")))
})
