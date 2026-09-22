// Run the probe under many compiler configurations and demand one answer.
//
// The probe's value is not that it compiles -- it is that a feature-dense
// program has exactly one correct output, so *any* configuration that produces a
// different one has found a wrong program. live-8 was exactly this shape: a
// scored decision family proposed an artifact that ran a call twice, and only a
// second configuration disagreeing revealed it.
//
//   node scripts/configs.mjs              # the standard matrix
//   node scripts/configs.mjs --keep       # leave the generated configs in place
import { mkdirSync, readFileSync, writeFileSync, rmSync, existsSync } from "node:fs"
import { spawnSync } from "node:child_process"
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const lilscriptRoot = process.env.LILSCRIPT_ROOT ?? resolve(root, "..", "lilscript")
const compiler = process.env.LILSCRIPT_COMPILER ?? resolve(lilscriptRoot, "target/release/lilscript")
const work = resolve(root, "dist", "configs")
mkdirSync(work, { recursive: true })

const base = readFileSync(resolve(root, "lilscript.toml"), "utf8")
const set = (key, value) => (text) =>
  new RegExp(`^${key}\\s*=.*$`, "m").test(text)
    ? text.replace(new RegExp(`^${key}\\s*=.*$`, "m"), `${key} = ${value}`)
    : text.replace(/^\[javascript\]$/m, `[javascript]\n${key} = ${value}`)
const compose =
  (...fns) =>
  (text) =>
    fns.reduce((acc, fn) => fn(acc), text)

// Each row is a configuration the shipped compiler can actually be asked for.
// They are not expected to produce the same *bytes* -- only the same behaviour.
const MATRIX = {
  shipped: (t) => t,
  level0: set("optimization_level", "0"),
  level5: set("optimization_level", "5"),
  level9: set("optimization_level", "9"),
  level15: set("optimization_level", "15"),
  searchOff: set("candidate_search", '"off"'),
  searchAlways: set("candidate_search", '"always"'),
  gzip: set("cost_model", '"gzip"'),
  raw: set("cost_model", '"raw"'),
  perfFirst: set("priority", '"performance-first"'),
  balanced: set("priority", '"balanced"'),
  noPeephole: set("optimizations", "[]"),
  arrows: set("function_spelling", '"arrow"'),
  functions: set("function_spelling", '"function"'),
  looseNames: set("stable_local_names", "false"),
  beam1: compose(set("candidate_beam_width", "1"), set("candidate_search", '"always"')),
  beam32: compose(set("candidate_beam_width", "32"), set("candidate_search", '"always"')),
  // `loop_spelling` is deliberately *not* here: it is not a config key. It is
  // `LoopSpelling::Auto` in the options and varied only by the
  // `loop-spelling-selection` decision family, so the `while` and `do{..}while`
  // shapes have to be provoked from the source instead -- see `whileShaped` in
  // the probe.
  // Phase 5 of the migration: the post-layout renamer. Same behaviour, new
  // identifiers -- the one configuration that must never print a program the
  // others disagree with.
  freqNames: set("name_ordering", '"frequency-desc"'),
  freqNamesSearchOff: compose(set("name_ordering", '"frequency-desc"'), set("candidate_search", '"off"')),
  idiomNames: set("name_ordering", '"idiom-converged"'),
  idiomNamesSearchOff: compose(set("name_ordering", '"idiom-converged"'), set("candidate_search", '"off"')),
  phiRegions: compose(
    set("local_phi_expression_regions", "true"),
    set("candidate_search", '"off"'),
  ),
}

const expected = readFileSync(resolve(root, "expected.out"), "utf8")
const runner = resolve(root, "dist", "run.cjs")
if (!existsSync(runner)) {
  writeFileSync(runner, `let n=0;globalThis.read=()=>{n+=1;return 7};require(process.argv[2]);\n`)
}

let failures = 0
let ran = 0
for (const [name, make] of Object.entries(MATRIX)) {
  const config = resolve(work, `${name}.toml`)
  writeFileSync(config, make(base))
  const out = resolve(work, `${name}.js`)
  const build = spawnSync(
    compiler,
    [resolve(root, "src/probe.lil"), "--target", "js", "--config", config, "-o", out],
    { encoding: "utf8" },
  )
  if (build.status !== 0) {
    console.error(`  ${name.padEnd(14)} DID NOT COMPILE: ${(build.stderr || "").split("\n")[0]}`)
    failures += 1
    continue
  }
  const run = spawnSync(process.execPath, [runner, out], { encoding: "utf8" })
  ran += 1
  if (run.status !== 0) {
    console.error(`  ${name.padEnd(14)} THREW: ${(run.stderr || "").split("\n")[0]}`)
    failures += 1
    continue
  }
  if (run.stdout !== expected) {
    failures += 1
    const want = expected.split("\n")
    const got = run.stdout.split("\n")
    const lines = []
    for (let i = 0; i < Math.max(want.length, got.length); i += 1) {
      if (want[i] !== got[i]) lines.push(`line ${i + 1}: want ${JSON.stringify(want[i])} got ${JSON.stringify(got[i])}`)
    }
    console.error(`  ${name.padEnd(14)} WRONG OUTPUT (${out})\n      ${lines.join("\n      ")}`)
    continue
  }
  console.log(`  ${name.padEnd(14)} ok  ${readFileSync(out).length} B`)
}

if (!process.argv.includes("--keep")) rmSync(work, { recursive: true, force: true })
console.log(`probelil: ${ran - failures} of ${Object.keys(MATRIX).length} configurations agree`)
process.exit(failures ? 1 : 0)
