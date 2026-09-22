// Record a receipt as accepted evidence for a milestone.
//
// The ledger is the place a claim becomes citable: it names which receipt backs
// which milestone and, through `requires`, exactly what that receipt is accepted
// for. `migration-progress.mjs` then refuses any claim the ledger cannot carry.
// Accepting is deliberately a separate act from producing a receipt, so a run
// that merely happened is not evidence until someone says what it proves.
//
//   node finer/tools/accept-evidence.mjs 001 <receipt.json> --requires cases,sourceBuilt,measurements --note "..."
//   node finer/tools/accept-evidence.mjs --remove 001 <receipt.json>
//   node finer/tools/accept-evidence.mjs --refresh          # re-pin every entry's hash after a rerun

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity } from "./artifact-evidence.mjs"
import { validateProgress } from "./migration-progress.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
const root = resolve(toolsDirectory, "../..")
const ledgerPath = join(root, "benchmarks/migration-results/accepted.json")

export const REQUIREMENTS = ["cases", "competitors", "sourceBuilt", "measurements"]

function readLedger() {
  if (!existsSync(ledgerPath)) return { schemaVersion: 1, milestones: {} }
  return JSON.parse(readFileSync(ledgerPath, "utf8"))
}

function writeLedger(ledger) {
  mkdirSync(dirname(ledgerPath), { recursive: true })
  writeFileSync(ledgerPath, JSON.stringify(ledger, null, 2) + "\n")
}

export function accept({ step, receiptPath, requires = [], note = null }) {
  const absolute = isAbsolute(receiptPath) ? receiptPath : resolve(receiptPath)
  if (!existsSync(absolute)) throw new Error(`no such receipt: ${receiptPath}`)
  for (const requirement of requires) {
    if (!REQUIREMENTS.includes(requirement)) throw new Error(`unknown requirement \`${requirement}\`; expected one of ${REQUIREMENTS.join(", ")}`)
  }
  const ledger = readLedger()
  const path = relative(root, absolute)
  const entry = { path, ...fileIdentity(absolute), requires, ...(note ? { note } : {}) }
  const milestone = ledger.milestones[step] ??= { receipts: [] }
  const at = milestone.receipts.findIndex(row => row.path === path)
  if (at >= 0) milestone.receipts[at] = entry
  else milestone.receipts.push(entry)
  milestone.receipts.sort((left, right) => left.path.localeCompare(right.path))
  writeLedger(ledger)
  return entry
}

export function remove({ step, receiptPath }) {
  const ledger = readLedger()
  const path = relative(root, isAbsolute(receiptPath) ? receiptPath : resolve(receiptPath))
  const milestone = ledger.milestones[step]
  if (!milestone) return false
  const before = milestone.receipts.length
  milestone.receipts = milestone.receipts.filter(row => row.path !== path)
  if (!milestone.receipts.length) delete ledger.milestones[step]
  writeLedger(ledger)
  return milestone.receipts.length !== before
}

/** Re-pin every entry to its receipt's current content. */
export function refresh() {
  const ledger = readLedger()
  const changed = []
  for (const [step, milestone] of Object.entries(ledger.milestones ?? {})) {
    for (const entry of milestone.receipts ?? []) {
      const absolute = join(root, entry.path)
      if (!existsSync(absolute)) { changed.push(`${step} ${entry.path}: missing`); continue }
      const identity = fileIdentity(absolute)
      if (identity.sha256 !== entry.sha256) {
        changed.push(`${step} ${entry.path}: re-pinned`)
        Object.assign(entry, identity)
      }
    }
  }
  writeLedger(ledger)
  return changed
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  const flag = name => { const at = argv.indexOf(name); return at < 0 ? null : argv[at + 1] }
  if (argv.includes("--refresh")) {
    const changed = refresh()
    console.log(changed.length ? changed.join("\n") : "every accepted receipt still matches its recorded hash")
  } else if (argv.includes("--remove")) {
    const [step, receiptPath] = argv.filter(value => !value.startsWith("--"))
    console.log(remove({ step, receiptPath }) ? `removed ${receiptPath} from ${step}` : "nothing to remove")
  } else {
    const positional = []
    for (let at = 0; at < argv.length; at += 1) {
      if (argv[at] === "--requires" || argv[at] === "--note") { at += 1; continue }
      if (!argv[at].startsWith("--")) positional.push(argv[at])
    }
    const [step, receiptPath] = positional
    if (!step || !receiptPath) { console.error("usage: accept-evidence.mjs <step> <receipt.json> [--requires a,b] [--note text]"); process.exit(2) }
    const entry = accept({ step, receiptPath, requires: (flag("--requires") ?? "").split(",").filter(Boolean), note: flag("--note") })
    console.log(`accepted for ${step}: ${entry.path} (${entry.bytes} bytes)`)
  }
  const result = validateProgress()
  if (!result.ok) {
    console.log(`\n${result.findings.length} unsupported claim(s) after this change:`)
    for (const finding of result.findings) console.log(`  - ${finding}`)
    process.exitCode = 1
  } else console.log("every progress claim is still supported by its accepted evidence")
}
