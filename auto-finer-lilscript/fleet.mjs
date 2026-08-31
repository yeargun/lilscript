#!/usr/bin/env node
// Fleet build-and-measure runner for the sibling LilScript ports.
//
// Every port ships its own `scripts/build.mjs --compile`, and each of those
// drives the compiler over a source tree big enough that one port can occupy
// a machine for minutes. Running them one at a time wastes most of the box;
// running them all at once makes each compile thrash, because the compiler
// itself is Rayon-parallel and will happily take every core.
//
// So this hands each port a fixed slice of cores (`taskset`) and tells the
// compiler to match (`RAYON_NUM_THREADS`), which keeps the slices from
// fighting. Sizes are then measured with `lilscript-codec` — the pinned
// zlib-1.3.1 / Brotli-1.1.0 encoders — never Node's, which disagree with the
// canonical encoder often enough to invalidate a comparison.
//
//   node auto-finer-lilscript/fleet.mjs                 # build + measure all
//   node auto-finer-lilscript/fleet.mjs --measure       # measure checked-in dist only
//   node auto-finer-lilscript/fleet.mjs --ports a,b     # a subset
//   node auto-finer-lilscript/fleet.mjs --slots 4       # concurrent ports
//   node auto-finer-lilscript/fleet.mjs --committed     # measure HEAD's dist, not the worktree's
import { spawn, spawnSync } from "node:child_process"
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { cpus } from "node:os"

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const siblings = resolve(repo, "..")
const codec = join(repo, "target/release/lilscript-codec")
const outDir = join(repo, "auto-finer-lilscript", "fleet-out")

const argv = process.argv.slice(2)
const flag = (name, fallback) => {
  const at = argv.indexOf(`--${name}`)
  return at === -1 ? fallback : argv[at + 1]
}
const has = (name) => argv.includes(`--${name}`)

const totalCores = cpus().length
const slots = Number(flag("slots", Math.max(1, Math.floor(totalCores / 2))))
const coresPerSlot = Math.max(1, Math.floor(totalCores / slots))
const buildTimeoutMs = Number(flag("timeout", 45 * 60)) * 1000

// Upstream baselines. `terserBrotli` values come from the markdown-stack
// harness, which minifies the real npm graph with a pinned Terser and measures
// it with this same codec, so they are directly comparable. A port with no
// entry here is measured but not judged — a missing baseline is reported as
// such rather than guessed at, because a wrong comparison is worse than none.
const BASELINES = {
  jquerylil: { artifact: "dist/jquery.esm.js", upstream: join(repo, "benchmarks/popular/node_modules/jquery/dist/jquery.min.js") },
  markedlil: { artifact: "dist/marked.esm.js", upstream: join(repo, "benchmarks/popular/node_modules/marked/marked.min.js") },
  mobxlil: { artifact: "dist/mobx.esm.js", upstream: join(repo, "benchmarks/popular/node_modules/mobx/dist/mobx.esm.production.min.js") },
  motionlil: { artifact: "dist/full.js", upstream: join(repo, "benchmarks/popular/node_modules/motion/dist/motion.js") },
  katexlil: { artifact: "dist/katex.esm.js", terserBrotli: 63137 },
  micromarklil: { artifact: "dist/micromark.esm.js", terserBrotli: 22776 },
  "mdast-util-from-markdownlil": { artifact: "dist/from-markdown.esm.js", terserBrotli: 23279 },
  "mdast-util-to-hastlil": { artifact: "dist/to-hast.esm.js", terserBrotli: 5016 },
  "hast-util-to-htmllil": { artifact: "dist/to-html.esm.js", terserBrotli: 9839 },
  "remark-parselil": { artifact: "dist/remark-parse.esm.js", terserBrotli: 23283 },
  "remark-rehypelil": { artifact: "dist/remark-rehype.esm.js", terserBrotli: 5061 },
  "remark-gfmlil": { artifact: "dist/remark-gfm.esm.js", terserBrotli: 11238 },
  "remark-mathlil": { artifact: "dist/remark-math.esm.js", terserBrotli: 2150 },
  "remark-breakslil": { artifact: "dist/remark-breaks.esm.js", terserBrotli: 1198 },
  "rehype-stringifylil": { artifact: "dist/rehype-stringify.esm.js", terserBrotli: 9886 },
  rehypelil: { artifact: "dist/rehype.esm.js", terserBrotli: 55080 },
  remarklil: { artifact: "dist/remark.esm.js", terserBrotli: 32551 },
  unifiedlil: { artifact: "dist/unified.esm.js", terserBrotli: 4425 },
  "rehype-katexlil": { artifact: "dist/rehype-katex.esm.js", terserBrotli: 113063 },
  "react-markdownlil": { artifact: "dist/react-markdown.esm.js", terserBrotli: 31092 },
}

function discoverPorts() {
  const requested = flag("ports", null)
  const names = spawnSync("ls", [siblings], { encoding: "utf8" }).stdout.split("\n")
    .filter((n) => /lil$|^lil-/.test(n) && n !== "lilscript")
    .filter((n) => existsSync(join(siblings, n, "scripts/build.mjs")))
  if (!requested) return names
  const want = new Set(requested.split(","))
  return names.filter((n) => want.has(n))
}

function measure(paths) {
  if (paths.length === 0) return {}
  const r = spawnSync(codec, ["--json", ...paths], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 })
  if (r.status !== 0) return {}
  const out = {}
  for (const a of JSON.parse(r.stdout).artifacts) out[a.path] = a
  return out
}

/** Read an artifact as committed at HEAD, so a dirty working tree cannot be
 *  mistaken for the project's state. Returns null when the path is not tracked. */
function committedCopy(port, relative) {
  const r = spawnSync("git", ["-C", join(siblings, port), "show", `HEAD:${relative}`],
    { encoding: "buffer", maxBuffer: 256 * 1024 * 1024 })
  if (r.status !== 0 || !r.stdout?.length) return null
  mkdirSync(outDir, { recursive: true })
  const at = join(outDir, `${port}__committed__${relative.replace(/\//g, "_")}`)
  writeFileSync(at, r.stdout)
  return at
}

function dirtiness(port) {
  const q = (paths) => spawnSync("git", ["-C", join(siblings, port), "status", "--short", "--", ...paths],
    { encoding: "utf8" }).stdout.trim().split("\n").filter(Boolean).length
  return { src: q(["src"]), config: q(["lilscript.toml", "."].slice(0, 1)), dist: q(["dist"]) }
}

function buildPort(port, slot) {
  return new Promise((done) => {
    const lo = slot * coresPerSlot
    const hi = lo + coresPerSlot - 1
    const started = Date.now()
    const child = spawn("taskset", ["-c", `${lo}-${hi}`, "node", "scripts/build.mjs", "--compile"], {
      cwd: join(siblings, port),
      env: { ...process.env, LILSCRIPT_ROOT: repo, RAYON_NUM_THREADS: String(coresPerSlot) },
      stdio: ["ignore", "pipe", "pipe"],
    })
    let err = ""
    child.stderr.on("data", (d) => { err += d.toString().slice(0, 4000) })
    child.stdout.on("data", () => {})
    const kill = setTimeout(() => child.kill("SIGKILL"), buildTimeoutMs)
    child.on("close", (code) => {
      clearTimeout(kill)
      done({ port, ok: code === 0, seconds: (Date.now() - started) / 1000, error: code === 0 ? null : err.trim().slice(-600) })
    })
  })
}

async function runPool(ports) {
  const queue = [...ports]
  const results = []
  const worker = async (slot) => {
    for (;;) {
      const port = queue.shift()
      if (!port) return
      process.stderr.write(`  [slot ${slot}] building ${port}\n`)
      const r = await buildPort(port, slot)
      process.stderr.write(`  [slot ${slot}] ${r.port} ${r.ok ? "ok" : "FAILED"} ${r.seconds.toFixed(0)}s\n`)
      results.push(r)
    }
  }
  await Promise.all(Array.from({ length: slots }, (_, s) => worker(s)))
  return results
}

const ports = discoverPorts()
if (ports.length === 0) { console.error("no ports found"); process.exit(1) }
mkdirSync(outDir, { recursive: true })

let builds = []
if (!has("measure")) {
  process.stderr.write(`building ${ports.length} ports, ${slots} slots x ${coresPerSlot} cores\n`)
  builds = await runPool(ports)
}
const buildBy = Object.fromEntries(builds.map((b) => [b.port, b]))

// Collect the artifact for every port, from HEAD or the working tree.
const wanted = []
for (const port of ports) {
  const base = BASELINES[port]
  if (!base) { wanted.push({ port, path: null }); continue }
  const local = join(siblings, port, base.artifact)
  const path = has("committed") ? committedCopy(port, base.artifact) : (existsSync(local) ? local : null)
  wanted.push({ port, path, upstream: base.upstream && existsSync(base.upstream) ? base.upstream : null, terserBrotli: base.terserBrotli })
}
const sizes = measure(wanted.flatMap((w) => [w.path, w.upstream].filter(Boolean)))

const rows = wanted.map((w) => {
  const lil = w.path ? sizes[w.path] : null
  const up = w.upstream ? sizes[w.upstream] : null
  const upstreamBrotli = up ? up.brotli11 : w.terserBrotli ?? null
  const delta = lil && upstreamBrotli != null ? lil.brotli11 - upstreamBrotli : null
  return {
    port: w.port,
    dirty: dirtiness(w.port),
    build: buildBy[w.port] ?? null,
    lil: lil ? { raw: lil.raw, gzip9: lil.gzip9, brotli11: lil.brotli11 } : null,
    upstreamBrotli,
    upstreamKind: up ? "npm-min" : w.terserBrotli != null ? "pinned-terser" : null,
    deltaBrotli: delta,
    verdict: delta == null ? "no-baseline" : delta < 0 ? "WIN" : delta === 0 ? "TIE" : "LOSS",
  }
})
rows.sort((a, b) => (b.deltaBrotli ?? -1e9) - (a.deltaBrotli ?? -1e9))

const wins = rows.filter((r) => r.verdict === "WIN").length
const losses = rows.filter((r) => r.verdict === "LOSS").length
const total = rows.filter((r) => r.deltaBrotli != null).reduce((s, r) => s + r.deltaBrotli, 0)

const pad = (s, n) => String(s).padEnd(n)
const num = (s, n) => String(s ?? "-").padStart(n)
let md = `# Fleet scoreboard\n\nGenerated: ${new Date().toISOString()}\n`
md += `Artifacts: ${has("committed") ? "committed (HEAD)" : "working tree"}. `
md += `Sizes from \`lilscript-codec\` (zlib 1.3.1 / Brotli 1.1.0).\n\n`
md += `**${wins} wins / ${losses} losses**, total Brotli delta **${total >= 0 ? "+" : ""}${total}** `
md += `(negative = LilScript smaller).\n\n`
md += `| port | src dirty | Lil raw | Lil gzip9 | Lil brotli11 | upstream brotli | delta | verdict | build |\n`
md += `|---|---:|---:|---:|---:|---:|---:|---|---|\n`
console.log(`${pad("port", 30)}${num("dirty", 6)}${num("brotli", 9)}${num("upstream", 10)}${num("delta", 8)}  verdict`)
for (const r of rows) {
  const b = r.build ? (r.build.ok ? `${r.build.seconds.toFixed(0)}s` : "FAILED") : "not built"
  md += `| ${r.port} | ${r.dirty.src} | ${num(r.lil?.raw, 1)} | ${num(r.lil?.gzip9, 1)} | ${num(r.lil?.brotli11, 1)} | ${num(r.upstreamBrotli, 1)} | ${r.deltaBrotli == null ? "-" : (r.deltaBrotli >= 0 ? "+" : "") + r.deltaBrotli} | ${r.verdict} | ${b} |\n`
  console.log(`${pad(r.port, 30)}${num(r.dirty.src, 6)}${num(r.lil?.brotli11, 9)}${num(r.upstreamBrotli, 10)}${num(r.deltaBrotli == null ? "-" : (r.deltaBrotli >= 0 ? "+" : "") + r.deltaBrotli, 8)}  ${r.verdict}`)
}
const failed = rows.filter((r) => r.build && !r.build.ok)
if (failed.length) {
  md += `\n## Build failures\n\n`
  for (const r of failed) md += `### ${r.port}\n\n\`\`\`\n${r.build.error}\n\`\`\`\n\n`
}
writeFileSync(join(outDir, "scoreboard.md"), md)
writeFileSync(join(outDir, "scoreboard.json"), JSON.stringify({ generatedAt: new Date().toISOString(), wins, losses, totalBrotliDelta: total, rows }, null, 2))
console.log(`\n${wins} wins / ${losses} losses, total Brotli ${total >= 0 ? "+" : ""}${total}`)
console.log(`wrote ${join(outDir, "scoreboard.md")}`)
