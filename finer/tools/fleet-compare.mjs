#!/usr/bin/env node
// Compare two fleet build arms port by port with the pinned codec.
//
//   node finer/tools/fleet-compare.mjs <distDirA> <distDirB> [--label-a X --label-b Y]
//                                      [--require-identical-unless-declared]
//                                      [--declared port,port] [--tolerance N]
//
// Each dist dir holds `<port>/…` as `workers.mjs build --dist-dir` brings it
// back. The artifact scored per port is the one the shared baselines table
// names.
//
// WHY THIS IS IN finer/tools/ AND NOT finer/out/
//
// This lived in `finer/out/048/`, which is git-ignored, with a hardcoded repo
// path and a regex scrape of fleet.mjs's baselines table — so a table entry
// whose spelling drifted would silently drop that port from the comparison
// instead of failing it. The table now has one home (baselines.mjs) and both
// tools read it.
//
// THE GATE
//
// `--require-identical-unless-declared` is what a neutral migration phase runs.
// The semantically empty perturbation band is roughly -125..+30 Brotli and the
// entire peephole is worth 189 bytes on markedlil, so a "small" delta cannot be
// told apart from a lost optimisation. A phase that claims to change nothing
// must therefore prove BYTE-IDENTITY, not similarity: any port not named in
// `--declared` whose artifact differs at all fails the run, in either direction.
//
// Without that flag this is a report, and it exits 0.

import { execFileSync } from "node:child_process"
import { existsSync, readFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { baselines } from "./baselines.mjs"

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..")
const codec = join(repo, "target/release/lilscript-codec")

const argv = process.argv.slice(2)
const flag = (name, fallback) => {
  const at = argv.indexOf(`--${name}`)
  return at === -1 ? fallback : argv[at + 1]
}
const has = (name) => argv.includes(`--${name}`)

const positional = argv.filter((a, i) => !a.startsWith("--") && !(i > 0 && argv[i - 1].startsWith("--") && !["--require-identical-unless-declared"].includes(argv[i - 1])))
const [dirA, dirB] = positional
if (!dirA || !dirB) {
  process.stderr.write("usage: fleet-compare.mjs <distDirA> <distDirB> [--require-identical-unless-declared]\n")
  process.exit(2)
}
const labelA = flag("label-a", "A")
const labelB = flag("label-b", "B")
const strict = has("require-identical-unless-declared")
const declared = new Set((flag("declared", "") || "").split(",").filter(Boolean))
const tolerance = Number(flag("tolerance", 0))

// Which compiler produced each arm? workers.mjs records this now; without it an
// A/B can silently be a comparison of two unknown binaries, because other
// sessions rebuild target/release/lilscript in this shared checkout.
function armCompiler(dir) {
  for (const candidate of [join(dir, "last-build.json"), join(repo, "finer/out/workers/last-build.json")]) {
    if (!existsSync(candidate)) continue
    try {
      return JSON.parse(readFileSync(candidate, "utf8")).compiler ?? null
    } catch {
      return null
    }
  }
  return null
}

const compilerA = armCompiler(resolve(dirA))
const compilerB = armCompiler(resolve(dirB))
if (compilerA?.sha256 && compilerB?.sha256 && compilerA.sha256 === compilerB.sha256) {
  process.stderr.write(`[fleet-compare] WARNING: both arms report compiler ${compilerA.sha256.slice(0, 12)} — this compares nothing\n`)
}

const table = baselines(repo)
const measure = (paths) => {
  if (!paths.length) return {}
  const out = JSON.parse(execFileSync(codec, ["--json", ...paths], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }))
  return Object.fromEntries(out.artifacts.map((a) => [a.path, a]))
}

const pairs = []
const missing = []
for (const [port, { artifact }] of Object.entries(table)) {
  const relative = artifact.replace(/^dist\//, "")
  const a = resolve(dirA, port, relative)
  const b = resolve(dirB, port, relative)
  if (!existsSync(a) || !existsSync(b)) {
    missing.push({ port, where: [!existsSync(a) && labelA, !existsSync(b) && labelB].filter(Boolean).join("+") })
    continue
  }
  pairs.push({ port, a, b })
}

const sizes = measure(pairs.flatMap((p) => [p.a, p.b]))
let net = 0
let violations = 0

const head = `${"port".padEnd(28)} ${labelA.padStart(9)} ${labelB.padStart(9)} ${"Δbrotli".padStart(8)} ${"Δraw".padStart(7)}  identical`
process.stdout.write(`${head}\n`)
for (const p of pairs) {
  const A = sizes[p.a]
  const B = sizes[p.b]
  const identical = readFileSync(p.a).equals(readFileSync(p.b))
  const dBrotli = B.brotli11 - A.brotli11
  net += dBrotli
  process.stdout.write(
    `${p.port.padEnd(28)} ${String(A.brotli11).padStart(9)} ${String(B.brotli11).padStart(9)} ` +
      `${String(dBrotli).padStart(8)} ${String(B.raw - A.raw).padStart(7)}  ${identical ? "yes" : "no"}\n`
  )
  if (!strict || identical) continue
  if (declared.has(p.port)) continue
  if (Math.abs(dBrotli) <= tolerance) continue
  violations += 1
}

for (const m of missing) process.stdout.write(`${m.port.padEnd(28)} missing in ${m.where}\n`)

process.stdout.write(`\nnet Brotli ${labelB} − ${labelA}: ${net} over ${pairs.length} ports\n`)
if (missing.length) process.stdout.write(`${missing.length} port(s) missing from an arm — a missing port is not a passing port\n`)

if (strict) {
  if (violations || missing.length) {
    process.stdout.write(
      `\nFAIL: ${violations} undeclared byte change(s), ${missing.length} missing.\n` +
        `A phase that claims to change nothing must be byte-identical: the noise band is wider than the\n` +
        `whole peephole is worth, so "close enough" cannot distinguish neutrality from a lost optimisation.\n`
    )
    process.exit(1)
  }
  process.stdout.write(`\nOK: byte-identical on every port except ${declared.size ? [...declared].join(", ") : "(none declared)"}\n`)
}
