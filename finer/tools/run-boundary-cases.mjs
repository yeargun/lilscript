// Run 002's boundary tables for the boundaries that have both an upstream
// oracle and a source-built candidate, and write one receipt each.
import { existsSync } from "node:fs"
import { createRequire } from "node:module"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { runBoundaryCases } from "./boundary-conformance.mjs"
import { HAST_TO_HTML_CASES, MDAST_TO_HAST_CASES } from "./boundary-cases-unist.mjs"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..")
// Which source-baseline run supplies the candidate artifacts.
const BASELINE_RUN = process.env.BASELINE_RUN ?? "2026-09-20-source-baselines"

const BOUNDARIES = [
  {
    boundary: "mdast-util-to-hastlil",
    upstreamFrom: "/home/azureuser/mdast-util-to-hastlil", upstreamPackage: "mdast-util-to-hast",
    candidate: join(root, "benchmarks/migration-results", BASELINE_RUN, "mdast-util-to-hastlil/to-hast.esm.js"),
    cases: MDAST_TO_HAST_CASES,
  },
  {
    boundary: "hast-util-to-htmllil",
    upstreamFrom: "/home/azureuser/hast-util-to-htmllil", upstreamPackage: "hast-util-to-html",
    candidate: join(root, "benchmarks/migration-results", BASELINE_RUN, "hast-util-to-htmllil/to-html.esm.js"),
    cases: HAST_TO_HTML_CASES,
  },
]

const runId = process.argv[2] ?? `${new Date().toISOString().slice(0, 10)}-boundary-conformance`
for (const entry of BOUNDARIES) {
  const require = createRequire(join(entry.upstreamFrom, "package.json"))
  const upstream = require.resolve(entry.upstreamPackage)
  if (!existsSync(entry.candidate)) { console.log(`${entry.boundary}: no source-built candidate at ${entry.candidate}`); continue }
  const receipt = await runBoundaryCases({
    boundary: entry.boundary, upstream, candidate: entry.candidate, cases: entry.cases,
    directory: join(root, "benchmarks/migration-results", runId, entry.boundary),
    notes: [`Candidate is the source-built artifact from benchmarks/migration-results/${BASELINE_RUN}/${entry.boundary}.`],
  })
  console.log(`\n=== ${entry.boundary} === ${receipt.counts.pass}/${receipt.counts.total} pass, ${receipt.counts.fail} fail, ${receipt.counts.broken} broken`)
  for (const row of receipt.cases) {
    if (row.status === "pass") continue
    console.log(`  ${row.status.toUpperCase()} ${row.id}`)
    console.log(`     upstream:  ${JSON.stringify(row.oracle).slice(0, 220)}`)
    console.log(`     candidate: ${JSON.stringify(row.candidate).slice(0, 220)}`)
  }
}
