// Validate the migration plan's progress table against accepted evidence.
//
// 001 asks for a receipt/progress validator built from the evidence primitives
// that already exist, with negative probes that reject stale artifacts, missing
// cases, changed installed dependencies or configuration, tampered hashes,
// empty competitor sets and checked milestones without accepted evidence.
//
// The plan itself stays the single source of truth for what is claimed; this
// tool only refuses claims the receipts do not support. It reads:
//
//   docs/migration/index.md                       the claimed state per milestone
//   benchmarks/migration-results/accepted.json    which receipt backs which claim
//   benchmarks/migration-results/<run>/...        the receipts themselves
//
// and reports one finding per unsupported claim. It never edits the plan.

import { existsSync, readFileSync } from "node:fs"
import { dirname, isAbsolute, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity } from "./artifact-evidence.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
export const REPOSITORY_ROOT = resolve(toolsDirectory, "../..")

// The nine states the plan declares. Anything else is a typo, not a state.
export const STATES = ["ready", "waiting", "active", "implemented", "verifying", "verified", "failed", "blocked", "stale"]
const SHA256 = /^[a-f0-9]{64}$/

/**
 * Parse the plan's `Progress and Dependencies` table.
 *
 * Returns rows of `{ step, title, done, prerequisites, state, receipt }`.
 * Throws when the table is missing or malformed, because a progress claim that
 * cannot be read is not a claim this tool may quietly pass.
 */
export function parsePlanTable(markdown) {
  const lines = markdown.split("\n")
  const start = lines.findIndex(line => /^\|\s*Done\s*\|\s*Step\s*\|/.test(line))
  if (start < 0) throw new Error("plan has no progress table")
  const rows = []
  for (const line of lines.slice(start + 2)) {
    if (!line.startsWith("|")) break
    const cells = line.split("|").slice(1, -1).map(cell => cell.trim())
    if (cells.length !== 5) throw new Error(`progress row has ${cells.length} cells, expected 5: ${line}`)
    const [done, step, prerequisites, state, receipt] = cells
    const named = /^\[(\d{3})\s+([^\]]+)\]\(#[^)]+\)$/.exec(step)
    if (!named) throw new Error(`progress row does not name a numbered step: ${step}`)
    if (!/^\[[ x]\]$/.test(done)) throw new Error(`progress row has an unreadable checkbox: ${done}`)
    rows.push({
      step: named[1], title: named[2], done: done === "[x]",
      prerequisites: prerequisites === "None" ? [] : [...prerequisites.matchAll(/\b(\d{3})\b/g)].map(match => match[1]),
      prerequisiteText: prerequisites,
      state: state.split(";")[0].trim(), stateText: state,
      receipt: receipt === "None" ? null : receipt,
    })
  }
  if (!rows.length) throw new Error("plan progress table has no rows")
  return rows
}

/** Read the accepted-evidence ledger, or an empty ledger when none exists. */
export function readLedger(root = REPOSITORY_ROOT) {
  const path = join(root, "benchmarks/migration-results/accepted.json")
  if (!existsSync(path)) return { schemaVersion: 1, milestones: {}, path, present: false }
  const ledger = JSON.parse(readFileSync(path, "utf8"))
  return { ...ledger, path, present: true }
}

function checkIdentity(root, entry, label, findings) {
  const path = isAbsolute(entry.path) ? entry.path : join(root, entry.path)
  if (!SHA256.test(entry.sha256 ?? "")) { findings.push(`${label}: recorded hash is not a SHA-256 digest`); return false }
  if (!existsSync(path)) { findings.push(`${label}: recorded file is missing: ${entry.path}`); return false }
  const actual = fileIdentity(path)
  if (actual.sha256 !== entry.sha256) { findings.push(`${label}: content changed since it was recorded: ${entry.path}`); return false }
  if (Number.isSafeInteger(entry.bytes) && actual.bytes !== entry.bytes) { findings.push(`${label}: byte count changed since it was recorded: ${entry.path}`); return false }
  return true
}

/**
 * Check one receipt as evidence for one claim.
 *
 * `requires` names the properties the claim depends on, so a receipt accepted
 * for a size comparison is not silently reused as behavior evidence.
 */
export function validateReceipt({ root = REPOSITORY_ROOT, entry, label }) {
  const findings = []
  const path = isAbsolute(entry.path) ? entry.path : join(root, entry.path)
  if (!existsSync(path)) return [`${label}: accepted receipt does not exist: ${entry.path}`]
  if (entry.sha256 && !checkIdentity(root, entry, `${label} receipt`, findings)) return findings
  let receipt
  try { receipt = JSON.parse(readFileSync(path, "utf8")) }
  catch (error) { return [`${label}: accepted receipt is not readable JSON: ${error.message}`] }

  const passed = receipt.passed === true || receipt.certification === "verified"
  if (!passed) findings.push(`${label}: receipt does not record a pass (passed=${JSON.stringify(receipt.passed)}, certification=${JSON.stringify(receipt.certification)})`)
  if (receipt.inputsStable === false) findings.push(`${label}: receipt records that its own inputs changed while it ran`)

  // Stale-input rejection. A receipt stays evidence only while the inputs it
  // pinned still hash as recorded; changed source, configuration or installed
  // dependencies invalidate the consumer rather than the receipt.
  for (const [property, rows] of [["inputs", receipt.inputs?.files], ["fixtures", receipt.fixtures], ["artifacts", receipt.requiredArtifacts]]) {
    for (const row of Array.isArray(rows) ? rows : []) checkIdentity(root, row, `${label} ${property}`, findings)
  }
  // A receipt produced by a named binary is evidence about that binary. If it
  // has been rebuilt since, the receipt describes a compiler that no longer
  // exists and the milestone is stale rather than verified.
  for (const property of ["compiler", "node"]) {
    const pinned = receipt[property]
    if (pinned?.path && pinned.sha256) checkIdentity(root, pinned, `${label} ${property}`, findings)
  }

  for (const requirement of entry.requires ?? []) {
    if (requirement === "cases") {
      const cases = receipt.requiredCases ?? receipt.observedCases ?? receipt.evidence?.cases
      if (!Array.isArray(cases) || !cases.length) findings.push(`${label}: accepted for case coverage but records no cases`)
      else if (cases.some(row => typeof row === "object" && row.status && row.status !== "pass")) findings.push(`${label}: accepted for case coverage but records a case that did not pass`)
    }
    if (requirement === "competitors") {
      const competitors = receipt.competitors ?? receipt.baselines
      if (!Array.isArray(competitors) || !competitors.length) findings.push(`${label}: accepted for a size comparison but names no eligible competitor`)
      else if (competitors.some(row => !row || typeof row.id !== "string" || !SHA256.test(row.sha256 ?? ""))) findings.push(`${label}: accepted for a size comparison but a competitor has no pinned artifact identity`)
    }
    if (requirement === "sourceBuilt" && receipt.sourceBuilt !== true) {
      findings.push(`${label}: accepted as a source-built baseline but the receipt records sourceBuilt=${JSON.stringify(receipt.sourceBuilt)}`)
    }
    if (requirement === "measurements") {
      const measurements = receipt.measurements ?? receipt.sizes
      if (!measurements || typeof measurements !== "object") findings.push(`${label}: accepted for delivered bytes but records no measurement`)
    }
  }
  return findings
}

/**
 * Reconcile plan, ledger and receipts. Returns `{ ok, rows, findings }`.
 *
 * A finding is an unsupported claim, not a compiler defect. The tool reports
 * every row so a reviewer can see what is claimed, not only what failed.
 */
export function validateProgress({ root = REPOSITORY_ROOT, markdown, ledger } = {}) {
  markdown ??= readFileSync(join(root, "docs/migration/index.md"), "utf8")
  ledger ??= readLedger(root)
  const rows = parsePlanTable(markdown)
  const findings = []
  const byStep = new Map(rows.map(row => [row.step, row]))

  const numbers = rows.map(row => row.step)
  if (new Set(numbers).size !== numbers.length) findings.push("progress table repeats a step number")
  if (numbers.join(",") !== [...numbers].sort().join(",")) findings.push("progress table is not in step order")

  for (const row of rows) {
    const label = `${row.step}`
    if (!STATES.includes(row.state)) findings.push(`${label}: \`${row.state}\` is not one of the declared states`)
    const accepted = ledger.milestones?.[row.step]?.receipts ?? []

    if (row.done) {
      // A checked box is the strongest claim the plan can make.
      if (!accepted.length) findings.push(`${label}: checked without accepted evidence in the ledger`)
      if (!row.receipt) findings.push(`${label}: checked but the plan names no accepted receipt`)
      if (row.state !== "verified") findings.push(`${label}: checked while its state is \`${row.state}\`; only \`verified\` may be checked`)
      for (const prerequisite of row.prerequisites) {
        const other = byStep.get(prerequisite)
        if (!other) findings.push(`${label}: names an unknown prerequisite ${prerequisite}`)
        else if (!other.done) findings.push(`${label}: checked while prerequisite ${prerequisite} is not`)
      }
    } else if (row.state === "verified") {
      findings.push(`${label}: state is \`verified\` but the box is not checked`)
    }

    for (const [index, entry] of accepted.entries()) {
      findings.push(...validateReceipt({ root, entry, label: `${label} receipt ${index + 1}` }))
    }
    if (accepted.length && row.receipt === null) findings.push(`${label}: the ledger accepts evidence the plan records as None`)
  }

  for (const step of Object.keys(ledger.milestones ?? {})) {
    if (!byStep.has(step)) findings.push(`ledger accepts evidence for ${step}, which the plan does not declare`)
  }
  return { ok: findings.length === 0, rows, findings, ledgerPresent: ledger.present !== false }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const result = validateProgress()
  const width = Math.max(...result.rows.map(row => row.title.length))
  for (const row of result.rows) {
    console.log(`${row.done ? "[x]" : "[ ]"} ${row.step} ${row.title.padEnd(width)}  ${row.state.padEnd(10)} ${row.receipt ?? "no accepted receipt"}`)
  }
  console.log("")
  if (result.ok) console.log("every progress claim is supported by its accepted evidence")
  else {
    console.log(`${result.findings.length} unsupported claim(s):`)
    for (const finding of result.findings) console.log(`  - ${finding}`)
    process.exitCode = 1
  }
}
