// The frozen qualification matrix and cost policy.
//
// 001 requires, before tuning: a finite non-empty boundary x release-profile x
// codec matrix including default/release routing, a definition of complete
// delivery accounting, numeric compile-time/RSS/runtime envelopes, warm and
// cold definitions, and sample/noise rules — with diagnostic profiles kept
// separate and the strict-win threshold left open.
//
// The matrix is derived from the frozen workload manifest rather than typed out,
// so a library, configuration or objective that is added to the manifest cannot
// quietly stay outside the qualification set. The envelopes are stated here as
// data, with the measurement each one came from, so raising a ceiling is a
// visible edit and not a silent adjustment after a slow run.

import { existsSync, readFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fingerprint } from "./artifact-evidence.mjs"

const toolsDirectory = dirname(fileURLToPath(import.meta.url))
export const REPOSITORY_ROOT = resolve(toolsDirectory, "../..")

export const CODECS = ["raw", "gzip", "brotli"]

// A profile is `release` when a published artifact is built with it, and
// `diagnostic` when it exists to explain a result. A diagnostic profile may
// never hold a cell that a release profile would lose: that is the rule this
// classification exists to make checkable, not a naming convention.
//
// Classification is by the profile's own distinguishing token, so a port that
// adds `lilscript.closed.toml` is classified without editing this table, and a
// token nobody has classified is reported rather than assumed to be a release.
export const PROFILE_ROLES = {
  // Release: each one builds bytes a consumer can fetch.
  "": "release-default",                       // lilscript.toml
  closed: "release-closed-world",
  "closed-world": "release-closed-world",
  "open-world": "release-open-world",
  app: "release-application",
  public: "release-public-api",
  raw: "release-raw-objective",
  bytes: "release-raw-objective",              // marked's raw-objective build
  gzip: "release-gzip-objective",
  "production.min": "release-minified",
  "reactivity-production": "release-variant",
  mhchem: "release-optional-extension",        // katex contrib module, its own boundary

  // Diagnostic: each one exists to explain a result, never to hold one.
  dev: "diagnostic-development",
  debug: "diagnostic-debug",
  none: "diagnostic-no-optimization",
  check: "diagnostic-check-only",              // optimization_level 0, cost_model raw
  nomangle: "diagnostic-mangling-fairness",
  perf: "diagnostic-performance",
  performance: "diagnostic-performance",
  "memory-parity": "diagnostic-memory-parity",
  "packs-safe": "diagnostic-packaging-variant",
  "sourcemap-analysis": "diagnostic-source-map",
  "sourcemap-hidden": "diagnostic-source-map",
  "sourcemap-inline": "diagnostic-source-map",
  "sourcemap-linked": "diagnostic-source-map",
}

/**
 * The token that names a profile's role, or null when the path is a test
 * fixture rather than a delivery profile.
 */
export function profileToken(path) {
  if (/(?:^|\/)test(?:s)?\//.test(path)) return null
  const name = path.split("/").pop()
  const match = /^lilscript(?:\.(.+))?\.toml$/.exec(name)
  if (match) return match[1] ?? ""
  const folder = /^(.+)\.toml$/.exec(name)
  return folder ? folder[1] : path
}

export function profileRole(path) {
  const token = profileToken(path)
  if (token === null) return "fixture"
  const role = PROFILE_ROLES[token]
  return role ?? `unclassified:${token}`
}

/**
 * Numeric envelopes, each with the observation that set it.
 *
 * These are ceilings for qualification, not predictions. A run that exceeds one
 * is an unqualified run with a recorded cost, not a slower pass.
 */
export const ENVELOPES = {
  compileWallSecondsPerProfile: {
    ceiling: 300,
    basis: "benchmarks/migration-results/2026-09-19-library-baselines/marked-release-baseline/README.md: the first Brotli-profile compile of Marked finished in 244.740 seconds and the four-profile build reached its 300-second limit during the next profile.",
    note: "Per profile, not per library. A library with four profiles needs four budgets, which is why whole-library builds are scheduled per profile.",
  },
  compileWallSecondsPerLibrary: {
    ceiling: 1500,
    basis: "Four release profiles at the per-profile ceiling, plus one profile's worth of packaging and measurement.",
    note: "Unqualified above this; the run is preserved with its partial artifacts marked ineligible.",
  },
  compilerResidentMegabytes: {
    ceiling: 6144,
    basis: "Host is a burstable 8-vCPU, 16 GiB machine; two concurrent compiles must fit with the test runner alongside.",
    note: "Measured as peak process RSS by GNU time, recorded per invocation.",
  },
  firstValidArtifactMilliseconds: {
    ceiling: 60_000,
    basis: "benchmarks/migration-results/2026-09-20-semantic-cli-cost/README.md: on the 12-module fixture the first qualified artifact appears internally after about 26 ms while the service takes 1,965 ms total.",
    note: "Time to the first eligible artifact, separate from time to the admitted incumbent.",
  },
  codecShareOfCompileWall: {
    ceiling: 0.95,
    basis: "Same study: canonical Brotli accounted for 1,877.934 ms of a 1,965.194 ms service wall, which is 95.6% and already at the ceiling.",
    note: "Above this, added probe budget is buying codec time rather than search. Reported per invocation; it bounds where effort may be added, not whether a build passes.",
  },
  runtimeRegressionRatio: {
    ceiling: 1.10,
    basis: "finer/status.md records cnlil shipping at 0.96-1.06 of upstream across nine harness lanes; a port slower than 1.10 on any lane is not a complete win.",
    note: "Per harness lane against the upstream library, median of interleaved rounds.",
  },
}

/** Warm and cold, defined once so two receipts mean the same thing. */
export const TIMING_RULES = {
  cold: "First invocation after the page cache has not seen the binary or inputs in this session: no warmup run, caches not primed.",
  warm: "After at least one discarded warmup invocation of the same binary on the same inputs, with the OS page cache holding them.",
  samples: 5,
  discardedWarmups: 1,
  statistic: "median",
  interleaving: "Alternate arms within one session rather than running all of one arm first; a host under changing load otherwise assigns its drift to whichever arm ran later.",
  threads: "RAYON_NUM_THREADS pinned for every timed comparison; recorded in the receipt.",
  noise: [
    "Deterministic codec byte differences are real at any magnitude; a byte delta needs no repetition.",
    "Timing needs repetition. A wall-clock difference smaller than the interquartile range of either arm is not a result.",
    "This host is a burstable instance whose throughput halves once CPU credits drain, so timed comparisons record the host and the elapsed session time.",
    "Instruction alignment moves hot-loop timings by up to 17% on this host, so a timing delta from a single build is compared against a no-op perturbation build before it is believed.",
  ],
}

/** What "delivered bytes" counts, so a cell cannot win by excluding its costs. */
export const DELIVERY_ACCOUNTING = {
  counts: [
    "The scored artifact itself.",
    "Every adapter, helper, shim and runtime support file the artifact needs to run.",
    "Every chunk a consumer must fetch to use the public API, including dynamically imported ones.",
    "Bundled dependencies. A dependency that is left external is counted only when the competitor also leaves it external.",
    "Initialization code that runs before the public API is usable.",
    "Required non-JavaScript resources the artifact loads at startup.",
  ],
  excludes: [
    "Source maps, type declarations and documentation, which are not fetched to run the library.",
    "Development-only files that no release entry point reaches.",
  ],
  rules: [
    "The competitor and the candidate must leave the same specifiers external, verified by comparing the published name sets of both artifacts.",
    "A reduced export set is its own boundary, not a whole-library win; the boundary is named by the names it publishes.",
    "An artifact that fails its boundary's required cases is ineligible at any size.",
    "Cross-codec wins do not transfer: each of raw, gzip and Brotli is selected independently.",
  ],
  openDecision: "D4's numeric strict-win threshold remains open: this policy fixes what is measured and counted, not how many bytes constitute a win.",
}

/**
 * Build the frozen matrix from the workload manifest.
 *
 * One row per boundary x profile x codec. `required` marks the cells a closure
 * claim must cover; a diagnostic profile contributes rows that are explanatory
 * and can never satisfy a release objective.
 */
export function buildMatrix({ root = REPOSITORY_ROOT } = {}) {
  const manifest = JSON.parse(readFileSync(join(root, "benchmarks/libraries/maintained-workloads.json"), "utf8"))
  const rows = []
  const boundaries = [...manifest.libraries, ...(manifest.additionalCoverage ?? [])]
  for (const library of boundaries) {
    const configurations = library.configurations?.length ? library.configurations : [{ path: "lilscript.toml" }]
    const objectives = library.objectives?.length ? library.objectives : CODECS
    for (const configuration of configurations) {
      const role = profileRole(configuration.path)
      // A configuration that belongs to a test, not a delivery, is not a
      // qualification cell. It stays visible in `fixtures` below.
      if (role === "fixture") continue
      for (const codec of objectives) {
        rows.push({
          boundary: library.id, kind: library.kind ?? "maintained-library", profile: configuration.path, role, codec,
          primary: codec === (library.primaryObjective ?? "brotli"),
          required: library.required !== false && role.startsWith("release"),
          worlds: library.worlds ?? ["open"],
          workspacePresent: existsSync(library.workspace ?? ""),
        })
      }
    }
  }
  return rows
}

/** Refuse a matrix that cannot serve as a qualification set. */
export function validateMatrix(rows) {
  const findings = []
  if (!rows.length) findings.push("the matrix is empty")
  const seen = new Set()
  for (const row of rows) {
    const key = `${row.boundary}|${row.profile}|${row.codec}`
    if (seen.has(key)) findings.push(`duplicate cell: ${key}`)
    seen.add(key)
    if (!CODECS.includes(row.codec)) findings.push(`${key}: ${row.codec} is not a declared codec`)
    if (row.role.startsWith("unclassified:")) findings.push(`${key}: profile token \`${row.role.slice("unclassified:".length)}\` has no declared role; classify it in PROFILE_ROLES before it can carry a cell`)
    if (row.required && !row.workspacePresent) findings.push(`${key}: required boundary has no workspace on disk`)
  }
  for (const boundary of new Set(rows.map(row => row.boundary))) {
    const own = rows.filter(row => row.boundary === boundary)
    if (!own.some(row => row.primary)) findings.push(`${boundary}: no primary objective among its cells`)
    if (!own.some(row => row.role.startsWith("release"))) findings.push(`${boundary}: every profile is diagnostic, so nothing is qualified`)
    if (!own.some(row => row.required)) continue
  }
  return findings
}

/** Configurations that belong to a test rather than to a delivery. */
export function boundaryFixtures({ root = REPOSITORY_ROOT } = {}) {
  const manifest = JSON.parse(readFileSync(join(root, "benchmarks/libraries/maintained-workloads.json"), "utf8"))
  return [...manifest.libraries, ...(manifest.additionalCoverage ?? [])]
    .flatMap(library => (library.configurations ?? []).map(configuration => configuration.path))
    .filter(path => profileRole(path) === "fixture")
}

export function policy({ root = REPOSITORY_ROOT } = {}) {
  const matrix = buildMatrix({ root })
  const findings = validateMatrix(matrix)
  const required = matrix.filter(row => row.required)
  const record = {
    schemaVersion: 1, kind: "cost-and-qualification-policy", generated: new Date().toISOString(),
    // The policy passes when every declared cell is classified. It is not a
    // compiler result and carries no size or speed claim of its own.
    passed: true,
    codecs: CODECS, profileRoles: PROFILE_ROLES, envelopes: ENVELOPES, timing: TIMING_RULES,
    delivery: DELIVERY_ACCOUNTING,
    matrix: {
      cells: matrix.length, requiredCells: required.length,
      boundaries: new Set(matrix.map(row => row.boundary)).size,
      releaseProfiles: [...new Set(matrix.filter(row => row.role.startsWith("release")).map(row => row.profile))].sort(),
      diagnosticProfiles: [...new Set(matrix.filter(row => row.role.startsWith("diagnostic")).map(row => row.profile))].sort(),
      fixtureConfigurations: [...new Set(boundaryFixtures({ root }))].sort(),
      rows: matrix,
    },
    findings,
  }
  record.passed = findings.length === 0
  record.fingerprint = fingerprint({ ...record, generated: null })
  return record
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const record = policy()
  const argv = process.argv.slice(2)
  const at = argv.indexOf("--out")
  if (at >= 0) {
    const { writeFileSync, mkdirSync } = await import("node:fs")
    const path = resolve(argv[at + 1])
    mkdirSync(dirname(path), { recursive: true })
    writeFileSync(path, JSON.stringify(record, null, 2) + "\n")
  }
  console.log(`${record.matrix.boundaries} boundaries, ${record.matrix.cells} cells (${record.matrix.requiredCells} required)`)
  console.log(`release profiles:    ${record.matrix.releaseProfiles.join(", ")}`)
  console.log(`diagnostic profiles: ${record.matrix.diagnosticProfiles.join(", ") || "none"}`)
  console.log(`fingerprint: ${record.fingerprint}`)
  if (record.findings.length) {
    console.log(`\n${record.findings.length} finding(s):`)
    for (const finding of record.findings) console.log(`  - ${finding}`)
    process.exitCode = 1
  } else console.log("\nthe matrix is finite, non-empty and every cell is classified")
}
