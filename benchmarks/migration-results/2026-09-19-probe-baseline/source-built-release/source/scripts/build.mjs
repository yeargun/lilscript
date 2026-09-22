// probelil: the migration's inner loop.
//
// One small library whose source deliberately exercises every language feature,
// so a miscompile shows up in seconds instead of in a 130-second port build.
// `--compile` rebuilds; with no flag it only re-checks the existing artifacts.
//
// It is a *differential* build, not just a compile: the same source is emitted
// at `preset = "none"` and at the shipped config, both are run, and both are
// compared against `expected.out`. A wrong program at either level fails here.
import { accessSync, constants, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { spawnSync } from "node:child_process"
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const lilscriptRoot = process.env.LILSCRIPT_ROOT ?? resolve(root, "..", "lilscript")
const dist = resolve(root, "dist")

function compilerPath() {
  const candidates = [
    process.env.LILSCRIPT_COMPILER,
    resolve(lilscriptRoot, "target/release/lilscript"),
    resolve(lilscriptRoot, "target/debug/lilscript"),
  ].filter(Boolean)
  for (const candidate of candidates) {
    try {
      accessSync(candidate, constants.X_OK)
      return candidate
    } catch {}
  }
  return null
}

const compiler = compilerPath()
if (!compiler) throw new Error("LilScript compiler not found; set LILSCRIPT_COMPILER")
mkdirSync(dist, { recursive: true })

// Both lanes name their config explicitly. Config discovery walks up from the
// *input*, so relying on it silently compiles with defaults on any machine that
// does not happen to have this file above the source — which is how a whole
// worker's worth of "passing" empty programs happened once already.
const lanes = [
  { name: "optimized", out: resolve(dist, "probe.js"), config: resolve(root, "lilscript.toml") },
  {
    name: "none",
    out: resolve(dist, "probe.none.js"),
    config: resolve(lilscriptRoot, "tests/config/no-optimization.toml"),
  },
]

if (process.argv.includes("--compile") || lanes.some((lane) => !existsSync(lane.out))) {
  for (const lane of lanes) {
    const result = spawnSync(
      compiler,
      [resolve(root, "src/probe.lil"), "--target", "js", "--config", lane.config, "-o", lane.out],
      { cwd: root, stdio: "inherit" },
    )
    if (result.status !== 0) {
      console.error(`probelil: ${lane.name} lane failed to compile`)
      process.exit(result.status ?? 1)
    }
  }
}

const expectedPath = resolve(root, "expected.out")
let failures = 0
const outputs = {}
// The probe's one host binding. `read()` is the opacity valve that stops the
// constant folder evaluating the whole program at compile time, so it has to be
// deterministic and it has to exist at run time.
const runner = resolve(dist, "run.cjs")
writeFileSync(
  runner,
  `let n=0;globalThis.read=()=>{n+=1;return 7};require(process.argv[2]);\n`,
)

for (const lane of lanes) {
  const run = spawnSync(process.execPath, [runner, lane.out], { cwd: root, encoding: "utf8" })
  if (run.status !== 0) {
    console.error(`probelil: ${lane.name} lane threw\n${run.stderr}`)
    failures += 1
    continue
  }
  outputs[lane.name] = run.stdout
}

if (process.argv.includes("--bless")) {
  writeFileSync(expectedPath, outputs.optimized ?? "")
  console.log(`probelil: blessed ${expectedPath}`)
  process.exit(0)
}

const expected = existsSync(expectedPath) ? readFileSync(expectedPath, "utf8") : null
if (expected === null) {
  console.error("probelil: no expected.out; run with --bless once the output is trusted")
  process.exit(1)
}

for (const [lane, actual] of Object.entries(outputs)) {
  if (actual === expected) continue
  failures += 1
  const want = expected.split("\n")
  const got = actual.split("\n")
  console.error(`probelil: ${lane} lane diverges from expected.out`)
  for (let index = 0; index < Math.max(want.length, got.length); index += 1) {
    if (want[index] !== got[index]) {
      console.error(`  line ${index + 1}: expected ${JSON.stringify(want[index])}, got ${JSON.stringify(got[index])}`)
    }
  }
}

if (failures) process.exit(1)
const size = readFileSync(lanes[0].out).length
console.log(`probelil: both lanes match expected.out (${expected.split("\n").length - 1} lines, ${size} B optimized)`)
