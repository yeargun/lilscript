#!/usr/bin/env node
// portgate — make the library ports gate the compiler.
//
// WHY THIS EXISTS
//
// As of 2026-09-04 nothing in this repository routinely proved that a compiler
// change preserves the behaviour of the shipped libraries:
//
//   * scripts/release-check.sh runs 74 stages and none of them is a port suite.
//   * 18 of 25 ports' `npm test` never rebuilds, so it validates a committed
//     dist/ produced by some earlier binary -- not the one under test.
//   * mobxlil's suite builds `--dev`; zodlil's builds lilscript.dev.toml at
//     optimization_level 8, below ParsedPeephole::minimum_level() == 9, so the
//     entire fold layer is switched off for the port whose fold miscompile
//     forced that config in the first place.
//   * katexlil's build script skips compilation on an mtime cache unless
//     `--force`, which once published a false "byte-identical" fleet row.
//
// Three fold miscompiles reached shipped artifacts through that gap. This tool
// closes it: it rebuilds every port from source with a NAMED compiler binary,
// refuses to believe a build that did not happen, records the failing-test SET
// rather than a pass/fail bit, and diffs two arms.
//
// USAGE
//
//   node finer/tools/portgate.mjs record  --arm baseline --compiler target/release/lilscript
//   node finer/tools/portgate.mjs record  --arm candidate --compiler /tmp/arms/cand/lilscript
//   node finer/tools/portgate.mjs compare finer/out/portgate/baseline finer/out/portgate/candidate
//
//   --ports a,b      restrict to named ports (default: every sibling *lil with a build script)
//   --out DIR        record destination (default finer/out/portgate/<arm>)
//   --skip-tests     build and measure only; useful while iterating on the compiler
//   --timeout S      per-port build timeout, seconds (default 2700)
//
// A `record` run exits non-zero if any port's build is UNTRUSTWORTHY -- see
// `classifyBuild`. It does NOT exit non-zero merely because tests failed: the
// failing set is data, and `compare` is what decides whether it got worse.
//
// This measures on THIS host. Per objective.md §9 the builds belong on the
// pool; this tool is the per-port contract that finer/tools/fleet.mjs should
// dispatch, not a replacement for it.

import { execFileSync, spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..")
const siblings = resolve(repo, "..")
const codec = join(repo, "target/release/lilscript-codec")

const argv = process.argv.slice(2)
const command = argv[0]
const flag = (name, fallback) => {
  const at = argv.indexOf(`--${name}`)
  return at === -1 ? fallback : argv[at + 1]
}
const has = (name) => argv.includes(`--${name}`)

const log = (line) => process.stderr.write(`[portgate] ${line}\n`)
const die = (message) => {
  log(message)
  process.exit(1)
}

// A build that finishes suspiciously fast, or without the timing line the
// compiler always prints under LILSCRIPT_TIMING, did not compile anything. That
// is a FAILED run, not a fast one -- treating it as a pass is exactly how a
// false byte-identical row gets published.
const MIN_TRUSTWORTHY_BUILD_MS = 10_000

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex")
}

function discoverPorts() {
  return readdirSync(siblings)
    .filter((name) => name.endsWith("lil") || name.startsWith("lil-"))
    .filter((name) => existsSync(join(siblings, name, "scripts/build.mjs")))
    .filter((name) => existsSync(join(siblings, name, "package.json")))
    .sort()
}

// Measure exact file bytes with the pinned encoders. Node's own zlib/brotli
// disagreed with these on 96 of 279 artifacts (objective.md §8), so nothing
// else may produce a published size.
function measure(paths) {
  if (!paths.length) return {}
  if (!existsSync(codec)) die(`${codec} is missing -- cargo build --release --bin lilscript-codec`)
  const out = execFileSync(codec, ["--json", ...paths], { encoding: "utf8", maxBuffer: 1 << 28 })
  const parsed = JSON.parse(out)
  // schemaVersion 1: { codecs: {...}, artifacts: [{ path, raw, gzip9, brotli11 }] }
  const rows = Array.isArray(parsed) ? parsed : (parsed.artifacts ?? [])
  if (!rows.length && paths.length) die("lilscript-codec returned no artifacts -- has its --json schema changed?")
  return Object.fromEntries(rows.map((r) => [resolve(r.path), r]))
}

function portArtifacts(port) {
  const dist = join(siblings, port, "dist")
  if (!existsSync(dist)) return []
  const found = []
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name)
      if (entry.isDirectory()) walk(path)
      else if (/\.(js|mjs|cjs)$/.test(entry.name) && !entry.name.endsWith(".map")) found.push(path)
    }
  }
  walk(dist)
  return found.sort()
}

// git state of the port checkout, so a comparison can say whether it is even
// looking at the same source. A dirty port is not disqualifying -- most are
// mid-migration -- but a comparison across two different dirty states is not a
// compiler measurement and must be reported as such.
function portSource(port) {
  const cwd = join(siblings, port)
  const git = (args) => {
    const r = spawnSync("git", args, { cwd, encoding: "utf8" })
    return r.status === 0 ? r.stdout.trim() : null
  }
  const dirty = git(["status", "--porcelain", "--", "src", "scripts", "package.json"])
  return {
    head: git(["rev-parse", "HEAD"]),
    dirtyFiles: dirty ? dirty.split("\n").filter(Boolean).length : null,
  }
}

const TIMING_KEYS = [
  "wall_ms",
  "emit_ms",
  "emit_calls",
  "codec_ms",
  "codec_calls",
  "peephole_ms",
  "optimize_ms",
  "lex_calls",
  "closers_calls",
  "idle_fold_calls",
  "active_fold_calls",
]

// The compiler prints one `lilscript-timing {...}` line per compile under
// LILSCRIPT_TIMING=1. A port that emits several artifacts prints several; sum
// the counters and keep the max wall, which is what "did this port's build cost
// more" means.
function parseTiming(text) {
  const lines = text.split("\n").filter((l) => l.includes("lilscript-timing {"))
  if (!lines.length) return null
  const totals = { lines: lines.length }
  for (const line of lines) {
    const at = line.indexOf("{")
    let row
    try {
      row = JSON.parse(line.slice(at))
    } catch {
      continue
    }
    for (const key of TIMING_KEYS) {
      if (typeof row[key] !== "number") continue
      if (key === "wall_ms") totals[key] = Math.max(totals[key] ?? 0, row[key])
      else totals[key] = (totals[key] ?? 0) + row[key]
    }
  }
  return totals
}

// The heart of the "do not believe a build that did not happen" rule.
function classifyBuild({ ok, elapsedMs, timing }) {
  if (!ok) return { trust: "failed", why: "build command exited non-zero" }
  if (!timing) {
    return {
      trust: "untrustworthy",
      why: "no lilscript-timing line: the compiler was never invoked (stale artifact, or a build script that caches on mtime -- pass --force)",
    }
  }
  if (elapsedMs < MIN_TRUSTWORTHY_BUILD_MS) {
    return {
      trust: "untrustworthy",
      why: `build finished in ${(elapsedMs / 1000).toFixed(1)}s, under the ${MIN_TRUSTWORTHY_BUILD_MS / 1000}s floor: almost certainly a cached or skipped compile`,
    }
  }
  return { trust: "ok", why: null }
}

// Failing-test SET, not a bit. `node --test` speaks TAP; jest prints ✕/●. Both
// are best-effort: the exit code is authoritative and the tail is always kept,
// so an unparsed runner degrades to "we know it failed" rather than to silence.
function extractFailures(text) {
  const failures = new Set()
  for (const line of text.split("\n")) {
    const tap = /^\s*not ok \d+\s*-?\s*(.+?)\s*$/.exec(line)
    if (tap) {
      failures.add(tap[1].trim())
      continue
    }
    const jest = /^\s*[✕×]\s+(.+?)(\s+\(\d+\s*ms\))?\s*$/.exec(line)
    if (jest) failures.add(jest[1].trim())
  }
  return [...failures].sort()
}

function runPort(port, { compiler, timeoutS, skipTests, logDir }) {
  const cwd = join(siblings, port)
  const env = {
    ...process.env,
    LILSCRIPT_COMPILER: compiler,
    LILSCRIPT_TIMING: "1",
  }

  // katexlil (and any future port with an mtime cache) skips compilation
  // without this; every other build script ignores an unknown flag or accepts
  // it, so probing the script text is cheaper than a per-port table.
  const script = readFileSync(join(cwd, "scripts/build.mjs"), "utf8")
  const buildArgs = ["scripts/build.mjs", "--compile"]
  if (script.includes('"--force"') || script.includes("'--force'")) buildArgs.push("--force")

  log(`${port}: building (${buildArgs.slice(1).join(" ")})`)
  const started = Date.now()
  const build = spawnSync("node", buildArgs, {
    cwd,
    env,
    encoding: "utf8",
    timeout: timeoutS * 1000,
    maxBuffer: 1 << 28,
  })
  const elapsedMs = Date.now() - started
  const buildText = `${build.stdout ?? ""}\n${build.stderr ?? ""}`
  writeFileSync(join(logDir, `${port}.build.log`), buildText)

  const timing = parseTiming(buildText)
  const verdict = classifyBuild({ ok: build.status === 0, elapsedMs, timing })

  const record = {
    port,
    source: portSource(port),
    build: {
      args: buildArgs,
      status: build.status,
      elapsedMs,
      trust: verdict.trust,
      why: verdict.why,
      timing,
      tail: buildText.trim().split("\n").slice(-12).join("\n"),
    },
    artifacts: [],
    tests: null,
  }

  if (verdict.trust === "ok") {
    const paths = portArtifacts(port)
    const sizes = measure(paths)
    record.artifacts = paths.map((path) => {
      const row = sizes[resolve(path)] ?? {}
      return {
        path: relative(join(siblings, port), path),
        sha256: sha256(path),
        raw: row.raw ?? statSync(path).size,
        gzip9: row.gzip9 ?? row.gzip ?? null,
        brotli11: row.brotli11 ?? row.brotli ?? null,
      }
    })
  }

  if (!skipTests && verdict.trust === "ok") {
    log(`${port}: testing`)
    const test = spawnSync("npm", ["test", "--silent"], {
      cwd,
      env,
      encoding: "utf8",
      timeout: timeoutS * 1000,
      maxBuffer: 1 << 28,
    })
    const testText = `${test.stdout ?? ""}\n${test.stderr ?? ""}`
    writeFileSync(join(logDir, `${port}.test.log`), testText)
    record.tests = {
      status: test.status,
      failing: extractFailures(testText),
      tail: testText.trim().split("\n").slice(-20).join("\n"),
    }
  }

  return record
}

function record() {
  const arm = flag("arm", "baseline")
  const compiler = resolve(flag("compiler", join(repo, "target/release/lilscript")))
  if (!existsSync(compiler)) die(`compiler ${compiler} does not exist`)

  const out = resolve(flag("out", join(repo, "finer/out/portgate", arm)))
  const logDir = join(out, "logs")
  mkdirSync(logDir, { recursive: true })

  const only = (flag("ports", "") || "").split(",").filter(Boolean)
  const ports = discoverPorts().filter((p) => !only.length || only.includes(p))
  if (!ports.length) die("no ports discovered")

  const timeoutS = Number(flag("timeout", 2700))
  const skipTests = has("skip-tests")

  // Copy the binary into the arm directory and record its digest. Two arms must
  // never reference target/release/lilscript, because a concurrent session
  // rebuilding it mid-run silently makes the comparison meaningless.
  const armCompiler = join(out, "lilscript")
  writeFileSync(armCompiler, readFileSync(compiler))
  execFileSync("chmod", ["+x", armCompiler])

  const manifest = {
    arm,
    compiler: { source: compiler, sha256: sha256(armCompiler) },
    repoHead: spawnSync("git", ["rev-parse", "HEAD"], { cwd: repo, encoding: "utf8" }).stdout?.trim(),
    host: { cpus: (process.report?.getReport?.()?.header?.cpus ?? []).length || null },
    ports: [],
  }
  log(`arm ${arm}: compiler ${manifest.compiler.sha256.slice(0, 12)}, ${ports.length} ports`)

  for (const port of ports) {
    const row = runPort(port, { compiler: armCompiler, timeoutS, skipTests, logDir })
    writeFileSync(join(out, `${port}.json`), `${JSON.stringify(row, null, 2)}\n`)
    manifest.ports.push(port)
    const t = row.build.trust
    const failing = row.tests?.failing?.length
    log(
      `${port}: build=${t}${t === "ok" ? ` ${(row.build.elapsedMs / 1000).toFixed(0)}s` : ` (${row.build.why})`}` +
        (row.tests ? ` tests=${row.tests.status === 0 ? "pass" : `${failing} failing`}` : "")
    )
  }

  writeFileSync(join(out, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`)

  const rows = manifest.ports.map((p) => JSON.parse(readFileSync(join(out, `${p}.json`), "utf8")))
  const untrusted = rows.filter((r) => r.build.trust !== "ok")
  log(`recorded ${rows.length} ports to ${out}`)
  if (untrusted.length) {
    log(`UNTRUSTWORTHY BUILDS (${untrusted.length}): ${untrusted.map((r) => r.port).join(", ")}`)
    log("a build that did not happen is a failed run, not a fast one")
    process.exit(1)
  }
}

function compare() {
  const [aDir, bDir] = argv.slice(1).filter((a) => !a.startsWith("--")).map((d) => resolve(d))
  if (!aDir || !bDir) die("usage: portgate.mjs compare <baselineDir> <candidateDir>")

  const readArm = (dir) => {
    const manifest = JSON.parse(readFileSync(join(dir, "manifest.json"), "utf8"))
    const ports = Object.fromEntries(
      manifest.ports.map((p) => [p, JSON.parse(readFileSync(join(dir, `${p}.json`), "utf8"))])
    )
    return { manifest, ports }
  }
  const a = readArm(aDir)
  const b = readArm(bDir)

  if (a.manifest.compiler.sha256 === b.manifest.compiler.sha256) {
    log("WARNING: both arms used the same compiler digest -- this compares nothing")
  }

  let regressions = 0
  const lines = []
  for (const port of Object.keys(a.ports)) {
    const left = a.ports[port]
    const right = b.ports[port]
    if (!right) {
      lines.push(`${port}: MISSING from candidate arm`)
      regressions += 1
      continue
    }
    if (left.build.trust !== "ok" || right.build.trust !== "ok") {
      lines.push(`${port}: build not trustworthy (${left.build.trust} -> ${right.build.trust})`)
      regressions += 1
      continue
    }

    // Bytes. Byte-identity is the gate for a phase that claims neutrality: the
    // semantically empty perturbation band is about -125..+30 Brotli, so a
    // "small" delta cannot be distinguished from a lost optimisation.
    const rightByPath = Object.fromEntries(right.artifacts.map((x) => [x.path, x]))
    for (const artifact of left.artifacts) {
      const other = rightByPath[artifact.path]
      if (!other) {
        lines.push(`${port}/${artifact.path}: artifact disappeared`)
        regressions += 1
        continue
      }
      if (other.sha256 === artifact.sha256) continue
      const delta = (other.brotli11 ?? 0) - (artifact.brotli11 ?? 0)
      const sign = delta > 0 ? "+" : ""
      lines.push(
        `${port}/${artifact.path}: bytes changed ${sign}${delta} brotli (${artifact.brotli11} -> ${other.brotli11})`
      )
      if (delta > 0) regressions += 1
    }

    // Behaviour. A failing set that grew is a regression; one that shrank is
    // progress; the same set is neutral even when non-empty, because most ports
    // are mid-migration and already red.
    const before = new Set(left.tests?.failing ?? [])
    const after = new Set(right.tests?.failing ?? [])
    const appeared = [...after].filter((t) => !before.has(t))
    const fixed = [...before].filter((t) => !after.has(t))
    if (appeared.length) {
      lines.push(`${port}: ${appeared.length} NEW failing tests: ${appeared.slice(0, 5).join(" | ")}`)
      regressions += 1
    }
    if (fixed.length) lines.push(`${port}: ${fixed.length} tests newly passing`)
    if ((left.tests?.status ?? 0) === 0 && (right.tests?.status ?? 0) !== 0 && !appeared.length) {
      lines.push(`${port}: suite exit ${left.tests.status} -> ${right.tests.status} with no parsed failure names`)
      regressions += 1
    }

    // Cost, per D9: record compile time with size.
    const lt = left.build.timing ?? {}
    const rt = right.build.timing ?? {}
    if (lt.emit_ms && rt.emit_ms) {
      const ratio = rt.emit_ms / lt.emit_ms
      if (ratio > 1.05 || ratio < 0.95) {
        lines.push(`${port}: emit_ms ${Math.round(lt.emit_ms)} -> ${Math.round(rt.emit_ms)} (${ratio.toFixed(2)}x)`)
      }
    }
  }

  process.stdout.write(`${lines.join("\n")}\n`)
  process.stdout.write(
    regressions
      ? `\nFAIL: ${regressions} regression(s) across ${Object.keys(a.ports).length} ports\n`
      : `\nOK: no regressions across ${Object.keys(a.ports).length} ports\n`
  )
  process.exit(regressions ? 1 : 0)
}

switch (command) {
  case "record":
    record()
    break
  case "compare":
    compare()
    break
  default:
    die("usage: portgate.mjs record --arm <name> --compiler <path> | portgate.mjs compare <dirA> <dirB>")
}
