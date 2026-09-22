// 005's measured baseline for the public semantic route.
//
// "Measure frontend-inclusive time to first valid artifact and RSS. This
// establishes a baseline, not a speed claim." Every case the new backend
// compiles is run through the real CLI (`--backend semantic`): the compiler's
// own `first_artifact_ns` (discovery, parse, check, lower, first admitted
// artifact) and, from GNU time, the whole process's wall clock and peak RSS.
// Timing follows the cost policy: one discarded warmup, five samples, median,
// one thread.

import { spawnSync } from "node:child_process"
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { fileIdentity, fingerprint } from "./artifact-evidence.mjs"
import { TIMING_RULES } from "./cost-policy.mjs"
import { preserveBinary } from "./preserved-binaries.mjs"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..")

const median = values => {
  const sorted = [...values].sort((left, right) => left - right)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
}
const percentile = (values, fraction) => {
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.min(sorted.length - 1, Math.floor(fraction * sorted.length))]
}

function sample({ compiler, source, target, output, config }) {
  const args = ["-f", "%e %M", compiler, source, "--backend", "semantic", "--target", target, "--mode", "development", "--config", config, "--explain", "json", "-o", output]
  // GNU time resolves wall clock to 10 ms, coarser than these compiles, so
  // wall time comes from this process's high-resolution clock; GNU time
  // still supplies the child's peak RSS.
  const clock = performance.now()
  const result = spawnSync("/usr/bin/time", args, { encoding: "utf8", env: { ...process.env, RAYON_NUM_THREADS: "1" }, maxBuffer: 1 << 26 })
  const wallMs = performance.now() - clock
  const lines = (result.stderr ?? "").trimEnd().split("\n")
  const [, rssKb] = lines.at(-1).split(" ").map(Number)
  let firstArtifactNs = null
  try { firstArtifactNs = JSON.parse(lines.slice(0, -1).join("\n")).first_artifact_ns ?? null } catch { firstArtifactNs = null }
  return { status: result.status, wallMs, rssMb: rssKb / 1024, firstArtifactMs: firstArtifactNs === null ? null : firstArtifactNs / 1e6 }
}

export function measureBaseline({ compiler, directory, config = join(root, "tests/config/no-optimization.toml") }) {
  const preserved = preserveBinary(compiler)
  mkdirSync(directory, { recursive: true })
  const cases = readdirSync(join(root, "tests/cases")).filter(name => name.endsWith(".lil")).sort()
  const rows = []
  for (const target of ["js-module", "c"]) {
    for (const name of cases) {
      const source = join(root, "tests/cases", name)
      const output = join(directory, `${name.slice(0, -4)}.${target === "c" ? "c" : "mjs"}`)
      const warmup = sample({ compiler: preserved.path, source, target, output, config })
      if (warmup.status !== 0) continue
      const samples = []
      for (let index = 0; index < TIMING_RULES.samples; index += 1) samples.push(sample({ compiler: preserved.path, source, target, output, config }))
      if (samples.some(row => row.status !== 0)) continue
      rows.push({
        case: name.slice(0, -4), target,
        firstArtifactMs: target === "c" ? null : median(samples.map(row => row.firstArtifactMs ?? NaN)),
        wallMs: median(samples.map(row => row.wallMs)),
        rssMb: median(samples.map(row => row.rssMb)),
        wallIqrMs: percentile(samples.map(row => row.wallMs), 0.75) - percentile(samples.map(row => row.wallMs), 0.25),
      })
    }
  }
  const summarize = (target, key) => {
    const values = rows.filter(row => row.target === target && Number.isFinite(row[key])).map(row => row[key])
    return values.length ? { cases: values.length, median: Number(median(values).toFixed(3)), p95: Number(percentile(values, 0.95).toFixed(3)), max: Number(Math.max(...values).toFixed(3)) } : null
  }
  const receipt = {
    schema: 1, kind: "semantic-service-baseline", created: new Date().toISOString(),
    compiler: preserved, config: { path: config, ...fileIdentity(config) }, timing: TIMING_RULES,
    scope: "every tests/cases program the semantic backend compiles; development mode; one thread; a baseline, not a speed claim",
    summary: {
      javascript: { firstArtifactMs: summarize("js-module", "firstArtifactMs"), wallMs: summarize("js-module", "wallMs"), rssMb: summarize("js-module", "rssMb") },
      native: { wallMs: summarize("c", "wallMs"), rssMb: summarize("c", "rssMb") },
    },
    rows,
    passed: rows.length > 0,
  }
  receipt.fingerprint = fingerprint({ ...receipt, created: null })
  writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n")
  return receipt
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  const flag = name => { const at = argv.indexOf(name); return at < 0 ? null : argv[at + 1] }
  const receipt = measureBaseline({
    compiler: resolve(flag("--compiler") ?? join(root, "target/release/lilscript")),
    directory: resolve(flag("--out") ?? join(root, `benchmarks/migration-results/${new Date().toISOString().slice(0, 10)}-semantic-service-baseline`)),
  })
  console.log(JSON.stringify(receipt.summary, null, 1))
}
