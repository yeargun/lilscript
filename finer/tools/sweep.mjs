#!/usr/bin/env node
// Parallel per-port config sweep.
//
// fleet.mjs builds every port once with whatever config is checked in. This
// asks a different question: for one port, which config actually produces the
// fewest Brotli bytes? Effort is not monotone -- unifiedlil is 4674 at level 13
// and 4696 at 15, and posthoglil's beam width runs 5668 / 5621 / 5736 / 5755 at
// widths 12 / 24 / 32 / 48 -- so the answer has to be measured, not reasoned to.
//
// Variants for one port are serialized (they share dist/), ports run in
// parallel, each pinned to its own core slice so the Rayon-parallel compiler
// inside each slot does not fight the others.
//
//   node finer/tools/sweep.mjs --ports posthoglil,unifiedlil
//   node finer/tools/sweep.mjs --ports jquerylil --variants base,l13
//   node finer/tools/sweep.mjs --slots 4 --timeout 3600
import { spawn, spawnSync } from "node:child_process"
import { readFileSync, writeFileSync, copyFileSync, existsSync, mkdirSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { cpus } from "node:os"

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..")
const siblings = resolve(repo, "..")
const codec = join(repo, "target/release/lilscript-codec")
const outDir = join(repo, "finer", "out", "sweep")

const argv = process.argv.slice(2)
const flag = (n, d) => { const i = argv.indexOf(`--${n}`); return i === -1 ? d : argv[i + 1] }

const totalCores = cpus().length
const slots = Number(flag("slots", 4))
const coresPerSlot = Math.max(1, Math.floor(totalCores / slots))
const timeoutMs = Number(flag("timeout", 3600)) * 1000

// Upstream Brotli bar per port: the size to beat. `artifact` is the file the
// port's own build writes and the site measures as its primary row.
const PORTS = {
  posthoglil: { artifact: "dist/posthog.esm.js", target: 5622 },
  unifiedlil: { artifact: "dist/unified.esm.js", target: 4425 },
  "remark-mathlil": { artifact: "dist/remark-math.esm.js", target: 2097 },
  jquerylil: { artifact: "dist/jquery.esm.js", target: 27445 },
  mobxlil: { artifact: "dist/mobx.esm.js", target: 12937 },
  katexlil: { artifact: "dist/katex.esm.js", target: 63044 },
  micromarklil: { artifact: "dist/micromark.esm.js", target: 22696 },
  "remark-parselil": { artifact: "dist/remark-parse.esm.js", target: 23171 },
  "mdast-util-from-markdownlil": { artifact: "dist/from-markdown.esm.js", target: 23151 },
  remarklil: { artifact: "dist/remark.esm.js", target: 22770 },
  "react-markdownlil": { artifact: "dist/react-markdown.esm.js", target: 31082 }
}

// Each variant is a function from the port's checked-in config text to the text
// to try. `base` measures the config as committed, so every run carries its own
// control rather than trusting a previously recorded number.
const set = (key, value) => (text) => {
  const line = `${key} = ${value}`
  return new RegExp(`^${key}\\s*=.*$`, "m").test(text)
    ? text.replace(new RegExp(`^${key}\\s*=.*$`, "m"), line)
    : text.replace(/^\[javascript\]$/m, `[javascript]\n${line}`)
}
const compose = (...fns) => (text) => fns.reduce((acc, fn) => fn(acc), text)
// Append a pass to an explicit `compression = [...]` allowlist. A port without one
// already gets the profile default, so there is nothing to add and the variant is
// reported as identical to base rather than silently measuring the same build twice.
// Set a key in `[optimization]` rather than `[javascript]`, creating the section
// if the port does not declare one.
const setOpt = (key, value) => (text) => {
  const line = `${key} = ${value}`
  if (new RegExp(`^${key}\\s*=.*$`, "m").test(text)) {
    return text.replace(new RegExp(`^${key}\\s*=.*$`, "m"), line)
  }
  return /^\[optimization\]$/m.test(text)
    ? text.replace(/^\[optimization\]$/m, `[optimization]\n${line}`)
    : `[optimization]\n${line}\n\n${text}`
}
const addPass = (pass) => (text) =>
  text.includes(`"${pass}"`) || !/^compression = \[$/m.test(text)
    ? text
    : text.replace(/^compression = \[$/m, `compression = [\n  "${pass}",`)

const VARIANTS = {
  base: (t) => t,
  always: set("candidate_search", '"always"'),
  l13: compose(set("optimization_level", "13"), set("candidate_search", '"always"')),
  l15: compose(set("optimization_level", "15"), set("candidate_search", '"always"')),
  beam24: compose(set("candidate_search", '"always"'), set("candidate_beam_width", "24")),
  beam24l13: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_beam_width", "24")),
  beam32: compose(set("candidate_search", '"always"'), set("candidate_beam_width", "32")),
  beam8: compose(set("candidate_search", '"always"'), set("candidate_beam_width", "8")),
  // Level 13 plus a tuned beam is the shape that has actually won so far, so the
  // grid walks the beam at 13 rather than reaching for 15 -- which measured worse
  // on unifiedlil (4696 against 4674) and is the objective's stated default anyway.
  b8: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_beam_width", "8")),
  b16: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_beam_width", "16")),
  b20: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_beam_width", "20")),
  b28: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_beam_width", "28")),
  b32: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_beam_width", "32")),
  b48: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_beam_width", "48")),
  l14: compose(set("optimization_level", "14"), set("candidate_search", '"always"')),
  pure: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("assume_pure_property_reads", "true")),
  proposals: compose(set("optimization_level", "13"), set("candidate_search", '"always"'), set("candidate_proposal_limit", "1536")),

  // jquerylil is the only port that enumerates `compression` explicitly, and its
  // list omits region-outlining -- the repeated-region outliner, which is the one
  // pass aimed squarely at emitted volume. It also does not set
  // local_phi_expression_regions, which config.rs records as -87 Brotli on this
  // exact artifact. Both are run at level 13 so a variant costs ten minutes
  // instead of the fifty the shipped level-15 config takes.
  jqL13: set("optimization_level", "13"),
  jqRegion: compose(set("optimization_level", "13"), addPass("region-outlining")),
  jqPhi: compose(set("optimization_level", "13"), set("local_phi_expression_regions", "true")),
  jqBoth: compose(set("optimization_level", "13"), addPass("region-outlining"), set("local_phi_expression_regions", "true")),
  jqRegion15: addPass("region-outlining"),
  jqBoth15: compose(addPass("region-outlining"), set("local_phi_expression_regions", "true")),

  // Specialisation is the suspected cause of the repetition class -- jquerylil and
  // remark-mathlil both emit *fewer* raw bytes than the competitor and still lose
  // Brotli, with roughly half the >=32-byte back-reference coverage (025). Cloning a
  // function per constant argument shortens each site and destroys the long identical
  // spans the compressor was paying almost nothing for. These ask the compiler to
  // keep one generic callee instead.
  nospec: setOpt("constant_parameter_specialization", "false"),
  nocallsite: setOpt("call_site_specialization", "false"),
  nocapture: setOpt("capture_signature_cloning", "false"),
  fold: setOpt("identical_function_folding", "true"),
  subsume: setOpt("function_subsumption", "true"),
  noclone: compose(setOpt("call_site_specialization", "false"), setOpt("capture_signature_cloning", "false"), setOpt("constant_parameter_specialization", "false")),
  foldsubsume: compose(setOpt("identical_function_folding", "true"), setOpt("function_subsumption", "true")),
  nofactory: setOpt("inline_closure_factories", "false"),
  // Terser puts 6975 identifier occurrences on one-character names in micromarklil
  // where we manage 4280, spending 3592 occurrences on two-character names against
  // its 513. `local_name_reserve` is the knob that decides how many short names are
  // held back, so sweep it rather than assume the default is right for every port.
  reserve0: set("local_name_reserve", "0"),
  reserve8: set("local_name_reserve", "8"),
  reserve48: set("local_name_reserve", "48"),
  reserve96: set("local_name_reserve", "96"),
  // micromarklil stops with work-budget-exhausted and 46 of 47 emission families
  // starved, including precise-cross-scope-shadowing -- the one that would stop
  // 62 of its 63 top-level bindings taking two-character names. Give the terminal
  // search room and see whether it reaches them.
  probe1536: set("terminal_codec_probe_limit", "1536"),
  probe4096: set("terminal_codec_probe_limit", "4096"),
  probe1536always: compose(set("candidate_search", '"always"'), set("terminal_codec_probe_limit", "1536"), set("candidate_proposal_limit", "1536")),
  // Hoisting a common subexpression into a temporary is a raw-byte win and can be a
  // compressed-byte loss: we emit `n=this.stack,r=n[n.length-1]` where Terser leaves
  // `this.stack[this.stack.length-1]` inline -- longer, but a phrase it repeats
  // verbatim elsewhere, so the copy costs a back-reference instead of its bytes.
  remat: set("rematerialize_member_reads", "true"),
  nocse: setOpt("common_subexpression_elimination", "false"),
  noscalar: setOpt("scalar_replacement", "false"),
  nocsescalar: compose(setOpt("common_subexpression_elimination", "false"), setOpt("scalar_replacement", "false"))
}

const portNames = (flag("ports", "") || Object.keys(PORTS).join(",")).split(",").filter(Boolean)
const variantNames = (flag("variants", "") || "base,always,l13,beam24,beam24l13").split(",").filter(Boolean)

function measure(path) {
  const out = spawnSync(codec, ["--json", path], { encoding: "utf8" })
  if (out.status !== 0) return null
  return JSON.parse(out.stdout).artifacts[0]
}

function build(port, cores, log) {
  return new Promise((done) => {
    // `scripts/build.mjs --compile`, never `npm run build`: several ports (jquerylil
    // among them) define `build` without `--compile`, so it re-bundles the cached
    // compiler output and returns in 0.15s. A config sweep driven through it reports
    // every variant as identical -- which is exactly what it did before this changed.
    const child = spawn(
      "taskset",
      ["-c", cores, "node", "scripts/build.mjs", "--compile"],
      {
        cwd: join(siblings, port),
        detached: true,
        env: { ...process.env, RAYON_NUM_THREADS: String(coresPerSlot), FORCE_COLOR: undefined },
        stdio: ["ignore", log, log]
      }
    )
    const timer = setTimeout(() => { try { process.kill(-child.pid, "SIGKILL") } catch {} }, timeoutMs)
    child.on("exit", (code) => { clearTimeout(timer); done(code) })
    child.on("error", () => { clearTimeout(timer); done(-1) })
  })
}

async function sweepPort(port, slot) {
  const spec = PORTS[port]
  if (!spec) return { port, error: "no baseline declared" }
  const root = join(siblings, port)
  if (!existsSync(root)) return { port, error: "missing sibling" }
  const configPath = join(root, "lilscript.toml")
  const original = readFileSync(configPath, "utf8")
  const first = slot * coresPerSlot
  const cores = `${first}-${first + coresPerSlot - 1}`
  const results = []
  for (const variant of variantNames) {
    const make = VARIANTS[variant]
    if (!make) continue
    const text = make(original)
    if (variant !== "base" && text === original) { results.push({ variant, note: "same as base" }); continue }
    writeFileSync(configPath, text)
    const started = Date.now()
    const logPath = join(outDir, `${port}-${variant}.log`)
    const log = (await import("node:fs")).openSync(logPath, "w")
    const code = await build(port, cores, log)
    ;(await import("node:fs")).closeSync(log)
    const seconds = Math.round((Date.now() - started) / 1000)
    if (code !== 0) { results.push({ variant, seconds, error: `build exit ${code}` }); continue }
    const sized = measure(join(root, spec.artifact))
    if (!sized) { results.push({ variant, seconds, error: "measure failed" }); continue }
    results.push({ variant, seconds, raw: sized.raw, brotli: sized.brotli11, delta: spec.target - sized.brotli11 })
    process.stderr.write(
      `[${port}] ${variant.padEnd(10)} brotli=${String(sized.brotli11).padStart(7)} ` +
      `vs ${spec.target} => ${sized.brotli11 <= spec.target ? "WIN " : "loss"} ${String(spec.target - sized.brotli11).padStart(7)}  ${seconds}s\n`
    )
  }
  writeFileSync(configPath, original)
  const scored = results.filter((r) => r.brotli !== undefined)
  scored.sort((a, b) => a.brotli - b.brotli)
  return { port, target: spec.target, best: scored[0] ?? null, results }
}

mkdirSync(outDir, { recursive: true })
const queue = [...portNames]
const all = []
await Promise.all(
  Array.from({ length: Math.min(slots, queue.length) }, async (_, slot) => {
    for (;;) {
      const port = queue.shift()
      if (!port) return
      all.push(await sweepPort(port, slot))
    }
  })
)

all.sort((a, b) => a.port.localeCompare(b.port))
const stamp = new Date().toISOString().replace(/[:.]/g, "-")
writeFileSync(join(outDir, `sweep-${stamp}.json`), JSON.stringify({ generatedAt: new Date().toISOString(), variants: variantNames, ports: all }, null, 2))
console.log(`\n${"port".padEnd(30)}${"best".padEnd(12)}${"brotli".padStart(8)}${"target".padStart(9)}${"delta".padStart(8)}  verdict`)
for (const row of all) {
  if (!row.best) { console.log(`${row.port.padEnd(30)}${(row.error ?? "no result").padEnd(12)}`); continue }
  const win = row.best.brotli <= row.target
  console.log(
    `${row.port.padEnd(30)}${row.best.variant.padEnd(12)}${String(row.best.brotli).padStart(8)}` +
    `${String(row.target).padStart(9)}${String(row.target - row.best.brotli).padStart(8)}  ${win ? "WIN" : "loss"}`
  )
}
console.log(`\nwrote ${join(outDir, `sweep-${stamp}.json`)}`)
