// Re-derive every pinned `terserBrotli` bar on one stated construction, so the
// scoreboard can be checked rather than trusted. Recipe, from 8.68/8.73:
//
//   the upstream package from npm
//   -> esbuild --bundle --format=esm, external exactly what our artifact imports
//   -> terser -c passes=3 -m --module        (compress defaults: 013 records the
//                                             baseline has `pure_getters` off and
//                                             `unsafe` is not eligible)
//   -> lilscript-codec brotli11
//
// A bar nobody can reconstruct is not a bar -- but a reconstruction that bundles
// a *different* program is not a check either. Several ports bundle a pinned
// source graph (their `source-graph.lock.json`) rather than the npm package's own
// dependency tree, and the npm entry then yields a much smaller program:
// mdast-util-from-markdownlil rebuilds at 55,519 raw against our 89,307 and the
// pinned bar's 83,876. So every row carries the raw sizes, and a row whose bar is
// more than 20% away from our artifact in raw bytes is reported as NOT
// COMPARABLE rather than as drift. Ports whose baseline is not a Terser number
// (posthoglil's Oxc kernel, the file-upstream ports) are skipped and say so.
import { readFileSync, existsSync, mkdirSync } from "node:fs"
import { execFileSync } from "node:child_process"
import { join } from "node:path"
import { homedir } from "node:os"
import { baselines } from "./baselines.mjs"

const repo = "/home/azureuser/lilscript"
const out = process.env.BAR_OUT ?? "/tmp/bars"
mkdirSync(out, { recursive: true })
const ESBUILD = join(homedir(), "unifiedlil/node_modules/.bin/esbuild")
const TERSER = join(homedir(), "unifiedlil/node_modules/.bin/terser")
const CODEC = join(repo, "target/release/lilscript-codec")
const br = (p) => JSON.parse(execFileSync(CODEC, ["--json", p]).toString()).artifacts[0].brotli11

// what our own artifact imports is what the bar must leave external
const externalsOf = (path) => {
  const src = readFileSync(path, "utf8")
  const found = new Set()
  for (const m of src.matchAll(/\bfrom\s*"([^"]+)"/g)) {
    if (!m[1].startsWith(".") && !m[1].startsWith("/")) found.add(m[1])
  }
  return [...found]
}

const entryOf = (dir) => {
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"))
  const exp = pkg.exports?.["."]
  const pick = (v) => (typeof v === "string" ? v : v?.import?.default ?? v?.import ?? v?.default)
  return pick(exp) ?? pkg.module ?? pkg.main ?? "index.js"
}

const rows = []
for (const [port, spec] of Object.entries(baselines(repo))) {
  if (spec.terserBrotli == null) { rows.push({ port, note: "not a Terser bar (file upstream)" }); continue }
  const upstream = port.replace(/lil$/, "")
  const dir = join(homedir(), port, "node_modules", upstream)
  const ours = join(homedir(), port, spec.artifact)
  if (!existsSync(dir) || !existsSync(ours)) { rows.push({ port, note: "upstream package not installed" }); continue }
  let entry
  try { entry = entryOf(dir) } catch { rows.push({ port, note: "no entry point" }); continue }
  const ext = externalsOf(ours).flatMap((e) => ["--external:" + e])
  const bundle = join(out, `${port}.bundle.js`), bar = join(out, `${port}.bar.js`)
  try {
    execFileSync(ESBUILD, [join(dir, entry), "--bundle", "--format=esm", ...ext, "--outfile=" + bundle], { stdio: "pipe" })
    execFileSync(TERSER, [bundle, "-c", "passes=3", "-m", "--module", "-o", bar], { stdio: "pipe" })
  } catch (error) { rows.push({ port, note: "build failed: " + String(error.message).split("\n")[0].slice(0, 60) }); continue }
  const barRaw = readFileSync(bar).length, oursRaw = readFileSync(ours).length
  // Two programs are comparable only if they are the same size *and* leave the
  // same things external. react-markdownlil's pinned bar externalises `react`
  // and `react/jsx-runtime` because our artifact imported them when it was
  // derived; the artifact imports nothing now, so that bar measures a program
  // missing React against one that carries it.
  const barExt = externalsOf(bar).sort().join(",")
  const oursExt = externalsOf(ours).sort().join(",")
  const comparable =
    Math.abs(barRaw - oursRaw) / Math.max(barRaw, oursRaw) <= 0.2 && barExt === oursExt
  rows.push({ port, pinned: spec.terserBrotli, rebuilt: br(bar), externals: ext.length,
              ours: br(ours), barRaw, oursRaw, comparable })
}

console.log(`${"port".padEnd(30)}${"pinned".padStart(9)}${"rebuilt".padStart(9)}${"drift".padStart(8)}${"barRaw".padStart(9)}${"oursRaw".padStart(9)}  verdict`)
for (const r of rows) {
  if (r.note) { console.log(`${r.port.padEnd(30)}${"".padStart(45)}  ${r.note}`); continue }
  const drift = r.rebuilt - r.pinned
  const verdict = !r.comparable
    ? `NOT COMPARABLE (different program or externals)`
    : Math.abs(drift) <= 250
      ? `reconstructs (drift ${drift})`
      : `PIN DRIFTED by ${drift}`
  console.log(
    `${r.port.padEnd(30)}${String(r.pinned).padStart(9)}${String(r.rebuilt).padStart(9)}` +
    `${String(drift > 0 ? "+" + drift : drift).padStart(8)}${String(r.barRaw).padStart(9)}` +
    `${String(r.oursRaw).padStart(9)}  ${verdict}`)
}
